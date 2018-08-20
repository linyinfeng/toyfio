#![feature(futures_api, pin, async_await, await_macro, arbitrary_self_types)]

use std::{io, ops::Deref, sync::Arc, rc::Rc, cell::RefCell, mem::PinMut, thread_local};
use futures::{
    Poll,
    future::{Future, FutureObj},
    task::{Context, Spawn, SpawnObjError, Wake, LocalWaker, local_waker}
};
use log::debug;

// Re-export modules exports
mod async_tcp_stream;
pub use crate::async_tcp_stream::AsyncTcpStream;

thread_local! {
    /// The global reactor.
    static REACTOR: Handle = Reactor::new(1024).expect("Failed to initialize thread local reactor");
}

/// Reactor for futures.
#[derive(Debug)]
struct Reactor {
    poll: mio::Poll,
    events: RefCell<mio::Events>,
    // Counter indicates the futures number associated with the reactor
    counter: RefCell<usize>,
}

/// Handle of `Reactor`.
///
/// Can be deref to a `Reactor`.
#[derive(Debug, Clone)]
struct Handle(Rc<Reactor>);

impl Deref for Handle {
    type Target = Reactor;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// The struct inside futures' waker.
#[derive(Debug, Clone, Copy)]
struct InnerWaker(usize);

impl InnerWaker {
    /// Leak `FutureObj` and store its pointer to InnerWaker.
    unsafe fn new(future: FutureObj<'static, ()>) -> InnerWaker {
        let boxed = Box::new(future);
        let pointer = Box::leak(boxed) as *mut _;
        let in_usize = pointer as usize;
        debug!("Leak FutureObj at {:?}", pointer);
        InnerWaker(in_usize)
    }

    /// Crate a local_waker from InnerWaker.
    fn local_waker(self) -> LocalWaker {
        unsafe { local_waker(Arc::new(self)) }
    }

    /// Convert leaked `FutureObj` back to a box.
    unsafe fn to_boxed_future_obj(self) -> Box<FutureObj<'static, ()>> {
        Box::from_raw(self.0 as *mut _)
    }

    /// Get leaked `FutureObj`'s mutable reference.
    unsafe fn to_future_obj_mut_ref(self) -> &'static mut FutureObj<'static, ()> {
        let boxed = self.to_boxed_future_obj();
        Box::leak(boxed) // Leak again
    }
}

impl Wake for InnerWaker {
    /// Wake the future which InnerWaker reference to.
    fn wake(arc_self: &Arc<InnerWaker>) {
        let waker = unsafe { local_waker(arc_self.clone()) };
        let mut handle = Reactor::handle();
        let mut context = Context::new(&waker, &mut handle);
        let future_obj = unsafe { arc_self.to_future_obj_mut_ref() };
        let future = PinMut::new(future_obj);
        match future.poll(&mut context) {
            Poll::Ready(_) => {
                debug!("Future done");
                unsafe { arc_self.to_boxed_future_obj() }; // Gain the box, auto drop it
                debug!("Drop FutureObj at {:p}", future_obj);
                REACTOR.with(|handle| {
                    handle.decrease_counter();
                });
            },
            Poll::Pending => {
                debug!("Future not yet ready");
            }
        }
    }
}

impl Reactor {
    /// Return thread local reactor handle.
    fn handle() -> Handle { REACTOR.with(Handle::clone) }

    /// Create a new reactor and return the handle, only called by thread local initialization.
    fn new(events_capacity: usize) -> Result<Handle, io::Error> {
        Ok(Handle(Rc::new(Self::new_reactor(events_capacity)?)))
    }

    /// Create a new reactor, only called by thread local initialization.
    fn new_reactor(events_capacity: usize) -> Result<Reactor, io::Error> {
        Ok(Reactor {
            poll: mio::Poll::new()?,
            events: RefCell::new(mio::Events::with_capacity(events_capacity)),
            counter: RefCell::new(0),
        })
    }

    /// Single iteration of event loop.
    fn iterate(&self) -> Result<(), io::Error> {
        debug!("Core iteration start");
        let mut events = self.events.borrow_mut();
        let _ready = self.poll.poll(&mut events, None)?;
        for event in &*events {
            let mio::Token(waker) = event.token();
            let waker = waker as *mut LocalWaker;
            debug!("Wake using waker at {:p}", waker);
            let boxed = unsafe { Box::from_raw(waker) };
            boxed.wake();
            debug!("Drop waker at {:p}", waker);
            // Waker will drop automatically
        }
        debug!("Core iteration end");
        Ok(())
    }

    /// Spawn the future and do event loop.
    fn start_loop(&self) -> Result<(), io::Error> {
        while *self.counter.borrow() > 0 {
            self.iterate()?;
        }
        Ok(())
    }

    /// Register when the handle first crated.
    /// 
    /// Use this function to eliminate the difference between first and other polls of future.
    fn register<E>(&self, handle: &E) -> Result<(), io::Error>
    where E: mio::Evented + ?Sized {
        // Use Token(0) to just hold the place
        self.poll.register(handle, mio::Token(0), mio::Ready::empty(), mio::PollOpt::oneshot())?;
        Ok(())
    }

    /// Manipulate waker and interest.
    fn reregister<E>(&self, handle: &E, waker: LocalWaker, interest: mio::Ready) -> Result<(), io::Error>
        where E: mio::Evented + ?Sized {
        let waker = Box::leak(Box::new(waker)) as *mut _; // Leak the waker
        debug!("Leak waker to {:p}", waker);
        let token = mio::Token(waker as usize);
        self.poll.reregister(handle, token, interest, mio::PollOpt::oneshot())?;
        Ok(())
    }

    // Deregister will be automatically performed when handle dropping
//    fn deregister<E>(&self, handle: &E) -> Result<(), io::Error>
//    where E: mio::Evented + ?Sized {
//        debug!("In function deregister");
//        self.poll.deregister(handle)?;
//        debug!("Out function deregister");
//        Ok(())
//    }

    /// Increase futures counter.
    /// 
    /// When a new future spawned, call this function.
    fn increase_counter(&self) {
        let mut counter = self.counter.borrow_mut();
        *counter += 1;
    }

    /// Decrease futures counter.
    /// 
    /// When a future done, call this function.
    /// 
    /// If the counter has been zero, the function will panic
    fn decrease_counter(&self) {
        let mut counter = self.counter.borrow_mut();
        if *counter > 0 {
            *counter -= 1;
        } else {
            panic!("Counter of reactor lower than 0");
        }
    }

    /// The real spawn function.
    fn do_spawn(&self, future: FutureObj<'static, ()>) {
        let waker = unsafe { InnerWaker::new(future) }.local_waker();
        waker.wake(); // Just wake it to do first polling
        self.increase_counter();
        // After that
    }
}

impl Spawn for Reactor {
    fn spawn_obj(&mut self, future: FutureObj<'static, ()>) -> Result<(), SpawnObjError> {
        // Never fail
        REACTOR.with(|handle| { handle.do_spawn(future) });
        Ok(())
    }
}

impl Spawn for Handle {
    fn spawn_obj(&mut self, future: FutureObj<'static, ()>) -> Result<(), SpawnObjError> {
        // Never fail
        REACTOR.with(|handle| { handle.do_spawn(future) });
        Ok(())
    }
}

/// Spawn a new future to the default reactor
pub fn spawn<F: Future<Output = ()> + Send + 'static>(f: F) {
    let future_obj = FutureObj::new(Box::new(f));
    REACTOR.with(|handle| { handle.do_spawn(future_obj) });
}

/// Spawn a new future and run the event loop
pub fn run<F: Future<Output = ()> + Send + 'static>(f: F) -> Result<(), io::Error> {
    spawn(f);
    REACTOR.with(|handle| { handle.start_loop() })
}