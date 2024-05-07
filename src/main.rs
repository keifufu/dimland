use clap::{Parser, Subcommand};
use lazy_static::lazy_static;
use smithay_client_toolkit::{
  compositor::{CompositorHandler, CompositorState},
  delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_shm,
  delegate_simple,
  output::{OutputHandler, OutputState},
  reexports::{
    client::{
      globals::{registry_queue_init, GlobalList},
      protocol::{
        wl_buffer::{self, WlBuffer},
        wl_output::WlOutput,
        wl_region::WlRegion,
        wl_shm::Format,
      },
      Connection, Dispatch, QueueHandle,
    },
    protocols::wp::viewporter::client::{
      wp_viewport::{self, WpViewport},
      wp_viewporter::{self, WpViewporter},
    },
  },
  registry::{ProvidesRegistryState, RegistryState, SimpleGlobal},
  registry_handlers,
  shell::{
    wlr_layer::{KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface},
    WaylandSurface,
  },
  shm::{raw::RawPool, Shm, ShmHandler},
};
use std::{
  env,
  process::{Command, Stdio},
  sync::{Arc, Condvar, Mutex},
};
use std::{fs, sync::Once};
use std::{
  io::{BufRead, BufReader, Write},
  os::unix::net::{UnixListener, UnixStream},
  process, thread,
};

const SOCKET_PATH: &str = "/tmp/dimland.sock";
const DEFAULT_ALPHA: f32 = 0.5;
const DEFAULT_RADIUS: u32 = 0;

static mut QH: Option<QueueHandle<DimlandData>> = None;
static QH_INIT: Once = Once::new();
const IS_DEBUG_BUILD: bool = cfg!(debug_assertions);

lazy_static! {
  static ref FLAG: Arc<(Mutex<bool>, Condvar)> = Arc::new((Mutex::new(false), Condvar::new()));
  static ref ARGS: Mutex<DimlandArgs> = Mutex::new(DimlandArgs {
    alpha: Some(DEFAULT_ALPHA),
    radius: Some(DEFAULT_RADIUS),
    command: None,
    detached: false
  });
}

#[derive(Debug, Subcommand, Clone)]
enum DimlandCommands {
  /// Stops the program
  Stop,
}

#[derive(Debug, Parser, Clone)]
#[command(version)]
struct DimlandArgs {
  #[arg(
    short,
    long,
    help = format!("0.0 is transparent, 1.0 is opaque, default is {DEFAULT_ALPHA}")
  )]
  alpha: Option<f32>,
  #[arg(
    short,
    long,
    help = format!("The radius of the opaque screen corners, default is {DEFAULT_RADIUS}")
  )]
  radius: Option<u32>,
  #[arg(short, long, hide = true)]
  detached: bool,
  #[command(subcommand)]
  command: Option<DimlandCommands>,
}

fn set_args(args: DimlandArgs) {
  let mut args_ref = ARGS.lock().unwrap();
  args_ref.alpha = args.alpha;
  args_ref.radius = args.radius;
  args_ref.command = args.command;
  args_ref.detached = args.detached;
  drop(args_ref);
}

fn get_args() -> DimlandArgs {
  let args_ref = ARGS.lock().unwrap();
  let cloned = args_ref.clone();
  drop(args_ref);
  cloned
}

fn main() {
  set_args(DimlandArgs::parse());
  let args = get_args();

  // ignore all signals
  ctrlc::set_handler(|| {}).expect("error setting signal handler");

  match args.command {
    Some(DimlandCommands::Stop) => {
      match UnixStream::connect(SOCKET_PATH) {
        Ok(mut stream) => stream.write_all("stop".as_bytes()).unwrap(),
        _ => (),
      };
      process::exit(0);
    }
    _ => (),
  }

  match UnixStream::connect(SOCKET_PATH) {
    Ok(mut stream) => {
      let message = env::args().collect::<Vec<String>>().join(" ");
      if let Err(err) = stream.write_all(message.as_bytes()) {
        eprintln!("Error sending IPC message: {}", err);
      }
      process::exit(0);
    }
    Err(_) => {
      if args.detached || IS_DEBUG_BUILD {
        cleanup();
        thread::spawn(listen_for_ipc);
        _main();
      } else {
        let exe_path = env::current_exe().unwrap();
        let path = exe_path.to_str().unwrap();
        let mut new_args: Vec<String> = env::args().collect();
        new_args.push("--detached".to_owned());
        Command::new(path)
          .args(&new_args[1..])
          .stdout(Stdio::null())
          .spawn()
          .unwrap();
        process::exit(0);
      }
    }
  };
}

fn listen_for_ipc() {
  let listener = match UnixListener::bind(SOCKET_PATH) {
    Ok(listener) => listener,
    Err(err) => {
      eprintln!("Failed to bind to socket: {}", err);
      cleanup();
      process::exit(1);
    }
  };

  for stream in listener.incoming() {
    match stream {
      Ok(stream) => {
        handle_ipc(stream);
      }
      Err(err) => {
        eprintln!("Error accepting connection: {}", err);
        break;
      }
    }
  }
}

fn handle_ipc(stream: UnixStream) {
  let mut reader = BufReader::new(stream);
  let mut message = String::new();

  match reader.read_line(&mut message) {
    Ok(_) => {
      if message == "stop" {
        cleanup();
        process::exit(0);
      }

      let args: Vec<String> = message
        .trim()
        .split_whitespace()
        .map(String::from)
        .collect();

      match DimlandArgs::try_parse_from(args) {
        Ok(args) => {
          set_args(args);
          let (lock, cvar) = &**FLAG;
          let mut flag_guard = lock.lock().unwrap();
          *flag_guard = true;
          cvar.notify_one();
        }
        _ => (),
      };
    }
    Err(err) => {
      eprintln!("Error reading message: {}", err);
    }
  }
}

fn cleanup() {
  if fs::metadata(SOCKET_PATH).is_ok() {
    if let Err(err) = fs::remove_file(SOCKET_PATH) {
      eprintln!("Error cleaning up socket file: {}", err);
      process::exit(1);
    }
  }
}

fn _main() {
  let args = get_args();
  let conn = Connection::connect_to_env().expect("where are you running this");

  let (globals, mut event_queue) = registry_queue_init(&conn).expect("queueless");

  QH_INIT.call_once(|| {
    let qh = event_queue.handle();
    unsafe {
      QH = Some(qh);
    }
  });

  let qh = unsafe { QH.as_ref().expect("QH not initialized") };

  let compositor = CompositorState::bind(&globals, &qh).expect("no compositor :sukia:");
  let layer_shell = LayerShell::bind(&globals, &qh).expect("huh?");
  let shm = Shm::bind(&globals, &qh).expect("wl_shm is not available");

  let alpha = args.alpha.unwrap_or(DEFAULT_ALPHA);
  let radius = args.radius.unwrap_or(DEFAULT_RADIUS);

  let mut data = DimlandData::new(compositor, &globals, &qh, layer_shell, alpha, radius, shm);

  let mut i = 0;
  loop {
    event_queue.roundtrip(&mut data).unwrap();

    if i > 10 {
      block_until_event();
      let new_args = get_args();
      data.alpha = new_args.alpha.unwrap_or(DEFAULT_ALPHA);
      data.radius = new_args.radius.unwrap_or(DEFAULT_RADIUS);
      data.rerender();
    } else {
      i += 1;
    }
  }
}

fn block_until_event() {
  let (lock, cvar) = &**FLAG;
  let mut flag_guard = lock.lock().unwrap();
  while !*flag_guard {
    flag_guard = cvar.wait(flag_guard).unwrap();
  }
  *flag_guard = false;
}

struct DimlandData {
  compositor: CompositorState,
  registry_state: RegistryState,
  output_state: OutputState,
  layer_shell: LayerShell,
  viewporter: SimpleGlobal<WpViewporter, 1>,
  alpha: f32,
  radius: u32,
  views: Vec<DimlandView>,
  exit: bool,
  shm: Shm,
  qh: &'static QueueHandle<Self>,
}

impl ShmHandler for DimlandData {
  fn shm_state(&mut self) -> &mut Shm {
    &mut self.shm
  }
}

struct DimlandView {
  first_configure: bool,
  width: u32,
  height: u32,
  buffer: WlBuffer,
  viewport: WpViewport,
  layer: LayerSurface,
  output: WlOutput,
}

fn create_buffer(
  alpha: f32,
  radius: u32,
  qh: &QueueHandle<DimlandData>,
  width: u32,
  height: u32,
  shm: &Shm,
) -> WlBuffer {
  let mut pool = RawPool::new(width as usize * height as usize * 4, shm).unwrap();
  let canvas = pool.mmap();

  // TODO: corner calc is kinda wrong?
  // see file:///stuff/screenshots/24-05-02T20-36-18.png
  // can't be bothered right now though for it is good enough

  {
    let corner_radius = radius;

    canvas
      .chunks_exact_mut(4)
      .enumerate()
      .for_each(|(index, chunk)| {
        let x = (index as u32) % width;
        let y = (index as u32) / width;

        let mut color = 0x00000000u32;
        let alpha = (alpha * 255.0) as u32;
        color |= alpha << 24;

        if (x < corner_radius
          && y < corner_radius
          && (corner_radius - x).pow(2) + (corner_radius - y).pow(2) > corner_radius.pow(2))
          || (x > width - corner_radius
            && y < corner_radius
            && (x - (width - corner_radius)).pow(2) + (corner_radius - y).pow(2)
              > corner_radius.pow(2))
          || (x < corner_radius
            && y > height - corner_radius
            && (corner_radius - x).pow(2) + (y - (height - corner_radius)).pow(2)
              > corner_radius.pow(2))
          || (x > width - corner_radius
            && y > height - corner_radius
            && (x - (width - corner_radius)).pow(2) + (y - (height - corner_radius)).pow(2)
              > corner_radius.pow(2))
        {
          color = 0xFF000000u32;
        }

        let array: &mut [u8; 4] = chunk.try_into().unwrap();
        *array = color.to_le_bytes();
      });
  }

  pool.create_buffer(
    0,
    width as i32,
    height as i32,
    width as i32 * 4,
    Format::Argb8888,
    (),
    qh,
  )
}

impl DimlandData {
  fn new(
    compositor: CompositorState,
    globals: &GlobalList,
    qh: &'static QueueHandle<Self>,
    layer_shell: LayerShell,
    alpha: f32,
    radius: u32,
    shm: Shm,
  ) -> Self {
    Self {
      compositor,
      registry_state: RegistryState::new(globals),
      output_state: OutputState::new(globals, qh),
      layer_shell,
      viewporter: SimpleGlobal::<wp_viewporter::WpViewporter, 1>::bind(globals, qh)
        .expect("wp_viewporter not available"),
      radius,
      alpha,
      views: Vec::new(),
      exit: false,
      shm,
      qh,
    }
  }

  fn create_view(&self, qh: &QueueHandle<Self>, output: WlOutput) -> DimlandView {
    let layer = self.layer_shell.create_layer_surface(
      qh,
      self.compositor.create_surface(qh),
      Layer::Overlay,
      Some("dimland_layer"),
      Some(&output),
    );

    let (width, height) = if let Some((width, height)) = self
      .output_state
      .info(&output)
      .and_then(|info| info.logical_size)
    {
      (width as u32, height as u32)
    } else {
      (0, 0)
    };

    layer.set_exclusive_zone(-1);
    layer.set_keyboard_interactivity(KeyboardInteractivity::None);
    let region = self.compositor.wl_compositor().create_region(qh, ());
    layer.set_input_region(Some(&region));
    layer.set_size(width, height);
    layer.commit();

    let viewport = self
      .viewporter
      .get()
      .expect("wp_viewporter failed")
      .get_viewport(layer.wl_surface(), qh, ());

    let buffer = create_buffer(self.alpha, self.radius, qh, width, height, &self.shm);

    DimlandView::new(qh, buffer, viewport, layer, output)
  }

  fn rerender(&mut self) {
    for view in &mut self.views {
      view.buffer = create_buffer(
        self.alpha,
        self.radius,
        self.qh,
        view.width,
        view.height,
        &self.shm,
      );
      view.first_configure = true;
      view.draw(self.qh);
    }
  }
}

impl DimlandView {
  fn new(
    _qh: &QueueHandle<DimlandData>,
    buffer: WlBuffer,
    viewport: WpViewport,
    layer: LayerSurface,
    output: WlOutput,
  ) -> Self {
    Self {
      first_configure: true,
      width: 0,
      height: 0,
      buffer,
      viewport,
      layer,
      output,
    }
  }

  fn draw(&mut self, _qh: &QueueHandle<DimlandData>) {
    if !self.first_configure {
      return;
    }

    self.layer.wl_surface().damage(
      0,
      0,
      self.width.try_into().unwrap(),
      self.height.try_into().unwrap(),
    );
    self.layer.wl_surface().attach(Some(&self.buffer), 0, 0);
    self.layer.commit();
  }
}

impl LayerShellHandler for DimlandData {
  fn closed(
    &mut self,
    _conn: &smithay_client_toolkit::reexports::client::Connection,
    _qh: &QueueHandle<Self>,
    _layer: &LayerSurface,
  ) {
    self.exit = true;
  }

  fn configure(
    &mut self,
    _conn: &smithay_client_toolkit::reexports::client::Connection,
    qh: &QueueHandle<Self>,
    layer: &LayerSurface,
    configure: smithay_client_toolkit::shell::wlr_layer::LayerSurfaceConfigure,
    _serial: u32,
  ) {
    let Some(view) = self.views.iter_mut().find(|view| &view.layer == layer) else {
      return;
    };

    (view.width, view.height) = configure.new_size;

    view
      .viewport
      .set_destination(view.width as _, view.height as _);

    if view.first_configure {
      view.draw(qh);
      view.first_configure = false;
    }
  }
}

impl OutputHandler for DimlandData {
  fn output_state(&mut self) -> &mut OutputState {
    &mut self.output_state
  }

  fn new_output(
    &mut self,
    _conn: &smithay_client_toolkit::reexports::client::Connection,
    qh: &QueueHandle<Self>,
    output: smithay_client_toolkit::reexports::client::protocol::wl_output::WlOutput,
  ) {
    self.views.push(self.create_view(qh, output));
  }

  fn update_output(
    &mut self,
    _conn: &smithay_client_toolkit::reexports::client::Connection,
    qh: &QueueHandle<Self>,
    output: smithay_client_toolkit::reexports::client::protocol::wl_output::WlOutput,
  ) {
    let new_view = self.create_view(qh, output);

    if let Some(view) = self.views.iter_mut().find(|v| v.output == new_view.output) {
      *view = new_view;
    }
  }

  fn output_destroyed(
    &mut self,
    _conn: &smithay_client_toolkit::reexports::client::Connection,
    _qh: &QueueHandle<Self>,
    output: smithay_client_toolkit::reexports::client::protocol::wl_output::WlOutput,
  ) {
    self.views.retain(|v| v.output != output);
  }
}

impl CompositorHandler for DimlandData {
  fn scale_factor_changed(
    &mut self,
    _conn: &smithay_client_toolkit::reexports::client::Connection,
    _qh: &QueueHandle<Self>,
    _surface: &smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface,
    _new_factor: i32,
  ) {
  }

  fn transform_changed(
    &mut self,
    _conn: &smithay_client_toolkit::reexports::client::Connection,
    _qh: &QueueHandle<Self>,
    _surface: &smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface,
    _new_transform: smithay_client_toolkit::reexports::client::protocol::wl_output::Transform,
  ) {
  }

  fn frame(
    &mut self,
    _conn: &smithay_client_toolkit::reexports::client::Connection,
    _qh: &QueueHandle<Self>,
    _surface: &smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface,
    _time: u32,
  ) {
  }
}

delegate_layer!(DimlandData);
delegate_output!(DimlandData);
delegate_registry!(DimlandData);
delegate_compositor!(DimlandData);
delegate_simple!(DimlandData, WpViewporter, 1);
delegate_shm!(DimlandData);

impl ProvidesRegistryState for DimlandData {
  fn registry(&mut self) -> &mut RegistryState {
    &mut self.registry_state
  }

  registry_handlers![OutputState];
}

impl Dispatch<WpViewport, ()> for DimlandData {
  fn event(
    _: &mut Self,
    _: &WpViewport,
    _: wp_viewport::Event,
    _: &(),
    _: &Connection,
    _: &QueueHandle<Self>,
  ) {
  }
}

impl Dispatch<WlBuffer, ()> for DimlandData {
  fn event(
    _: &mut Self,
    _: &WlBuffer,
    _: wl_buffer::Event,
    _: &(),
    _: &Connection,
    _: &QueueHandle<Self>,
  ) {
  }
}

impl Dispatch<WlRegion, ()> for DimlandData {
  fn event(
    _: &mut Self,
    _: &WlRegion,
    _: <WlRegion as smithay_client_toolkit::reexports::client::Proxy>::Event,
    _: &(),
    _: &Connection,
    _: &QueueHandle<Self>,
  ) {
  }
}

impl Drop for DimlandView {
  fn drop(&mut self) {
    self.viewport.destroy();
    self.buffer.destroy();
  }
}
