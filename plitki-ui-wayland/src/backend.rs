use std::{cell::RefCell, ffi::c_void, ops::Deref, rc::Rc};

use glium::{
    backend::{Backend, Context},
    SwapBuffersError,
};
use glutin::{
    platform::unix::RawContextExt, ContextBuilder, ContextError, ContextWrapper, PossiblyCurrent,
};
use slog_scope::debug;
use smithay_client_toolkit::reexports::client::{protocol::wl_surface::WlSurface, Display};
use takeable_option::Takeable;

pub struct GlutinRawBackendInner {
    context: Takeable<ContextWrapper<PossiblyCurrent, ()>>,
    dimensions: (u32, u32),
}

struct GlutinRawBackend(Rc<RefCell<GlutinRawBackendInner>>);

impl Deref for GlutinRawBackend {
    type Target = RefCell<GlutinRawBackendInner>;

    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}

// Based on https://github.com/glium/glium/blob/master/src/backend/glutin/mod.rs#L250
unsafe impl Backend for GlutinRawBackend {
    #[inline]
    fn swap_buffers(&self) -> Result<(), SwapBuffersError> {
        match self.borrow().context.swap_buffers() {
            Ok(()) => Ok(()),
            Err(ContextError::IoError(e)) => panic!("I/O Error while swapping buffers: {:?}", e),
            Err(ContextError::OsError(e)) => panic!("OS Error while swapping buffers: {:?}", e),
            // As of writing the FunctionUnavailable error is only thrown if
            // you are swapping buffers with damage rectangles specified.
            // Currently we don't support this so we just panic as this
            // case should be unreachable.
            Err(ContextError::FunctionUnavailable) => {
                panic!("function unavailable error while swapping buffers")
            }
            Err(ContextError::ContextLost) => Err(SwapBuffersError::ContextLost),
        }
    }

    #[inline]
    unsafe fn get_proc_address(&self, symbol: &str) -> *const c_void {
        self.borrow().context.get_proc_address(symbol) as *const _
    }

    #[inline]
    fn get_framebuffer_dimensions(&self) -> (u32, u32) {
        self.borrow().dimensions
    }

    #[inline]
    fn is_current(&self) -> bool {
        self.borrow().context.is_current()
    }

    #[inline]
    unsafe fn make_current(&self) {
        let context_takeable = &mut self.borrow_mut().context;
        let context = Takeable::take(context_takeable);
        let new_context = context.make_current().unwrap();
        Takeable::insert(context_takeable, new_context);
    }
}

impl GlutinRawBackendInner {
    #[inline]
    pub fn resize(&mut self, new_dimensions: (u32, u32)) {
        self.dimensions = new_dimensions;
        self.context.resize(new_dimensions.into());
    }
}

pub fn create_context(
    display: &Display,
    surface: &WlSurface,
    dimensions: (u32, u32),
) -> (Rc<RefCell<GlutinRawBackendInner>>, Rc<Context>) {
    let backend_inner = unsafe {
        let context = ContextBuilder::new()
            .build_raw_wayland_context(
                display.get_display_ptr() as _,
                surface.as_ref().c_ptr() as _,
                dimensions.0,
                dimensions.1,
            )
            .expect("Failed to create an OpenGL context!")
            .make_current()
            .unwrap();

        Rc::new(RefCell::new(GlutinRawBackendInner {
            context: Takeable::new(context),
            dimensions,
        }))
    };
    debug!("created a context"; "pixel_format" => ?backend_inner.borrow().context.get_pixel_format());

    let backend = GlutinRawBackend(backend_inner.clone());
    let context = unsafe {
        Context::new(backend, false, Default::default()).expect("Failed to create glium context")
    };

    (backend_inner, context)
}
