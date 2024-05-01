use clap::Parser;
use smithay_client_toolkit::{
  compositor::{CompositorHandler, CompositorState},
  delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_simple,
  output::{OutputHandler, OutputState},
  reexports::{
    client::{
      globals::{registry_queue_init, GlobalList},
      protocol::{
        wl_buffer::{self, WlBuffer},
        wl_output::WlOutput,
        wl_region::WlRegion,
      },
      Connection, Dispatch, QueueHandle,
    },
    protocols::wp::{
      single_pixel_buffer::v1::client::wp_single_pixel_buffer_manager_v1::{
        self, WpSinglePixelBufferManagerV1,
      },
      viewporter::client::{
        wp_viewport::{self, WpViewport},
        wp_viewporter::{self, WpViewporter},
      },
    },
  },
  registry::{ProvidesRegistryState, RegistryState, SimpleGlobal},
  registry_handlers,
  shell::{
    wlr_layer::{KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface},
    WaylandSurface,
  },
};

pub const DEFAULT_ALPHA: f32 = 0.5;

#[derive(Debug, Parser)]
#[command(version)]
pub struct DimlandArgs {
  #[arg(
    short,
    long,
    help = format!("0.0 is transparent, 1.0 is opaque, default is {DEFAULT_ALPHA}")
  )]
  pub alpha: Option<f32>,
}

fn main() {
  let args = DimlandArgs::parse();

  let conn = Connection::connect_to_env().expect("where are you running this");

  let (globals, mut event_queue) = registry_queue_init(&conn).expect("queueless");
  let qh = event_queue.handle();

  let compositor = CompositorState::bind(&globals, &qh).expect("no compositor :sukia:");
  let layer_shell = LayerShell::bind(&globals, &qh).expect("huh?");

  let alpha = args.alpha.unwrap_or(DEFAULT_ALPHA);
  let mut data = DimlandData::new(compositor, &globals, &qh, layer_shell, alpha);

  while !data.should_exit() {
    event_queue.blocking_dispatch(&mut data).expect("sus");
  }
}

pub struct DimlandData {
  compositor: CompositorState,
  registry_state: RegistryState,
  output_state: OutputState,
  layer_shell: LayerShell,
  pixel_buffer_mgr: SimpleGlobal<WpSinglePixelBufferManagerV1, 1>,
  viewporter: SimpleGlobal<WpViewporter, 1>,
  alpha: f32,
  views: Vec<DimlandView>,
  exit: bool,
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

impl DimlandData {
  pub fn new(
    compositor: CompositorState,
    globals: &GlobalList,
    qh: &QueueHandle<Self>,
    layer_shell: LayerShell,
    alpha: f32,
  ) -> Self {
    Self {
      compositor,
      registry_state: RegistryState::new(globals),
      output_state: OutputState::new(globals, qh),
      layer_shell,
      pixel_buffer_mgr: SimpleGlobal::<WpSinglePixelBufferManagerV1, 1>::bind(globals, qh)
        .expect("wp_single_pixel_buffer_manager_v1 not available!"),
      viewporter: SimpleGlobal::<wp_viewporter::WpViewporter, 1>::bind(globals, qh)
        .expect("wp_viewporter not available"),

      alpha,
      views: Vec::new(),

      exit: false,
    }
  }

  pub fn should_exit(&self) -> bool {
    self.exit
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

    let alpha = (u32::MAX as f32 * self.alpha) as u32;
    let buffer = self
      .pixel_buffer_mgr
      .get()
      .expect("failed to get buffer")
      .create_u32_rgba_buffer(0, 0, 0, alpha, qh, ());

    DimlandView::new(qh, buffer, viewport, layer, output)
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

impl Dispatch<WpSinglePixelBufferManagerV1, ()> for DimlandData {
  fn event(
    _: &mut Self,
    _: &WpSinglePixelBufferManagerV1,
    _: wp_single_pixel_buffer_manager_v1::Event,
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
