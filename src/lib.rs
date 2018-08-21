#![feature(futures_api, pin, async_await, await_macro, arbitrary_self_types)]

use std::{io, ops::Deref, sync::Arc, rc::Rc, cell::RefCell, mem::PinMut, thread_local};
use futures::{
    Poll,
    future::{Future, FutureObj},
    task::{Context, Spawn, SpawnObjError, Wake, LocalWaker, local_waker}
};
use slab::Slab;
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
    waker_storage: RefCell<Slab<LocalWaker>>,
    future_storage: RefCell<Slab<FutureObj<'static, ()>>>,
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
    /// Store `FutureObj` to `REACTOR`'s `future_storage`.
    fn new(future: FutureObj<'static, ()>) -> InnerWaker {
        InnerWaker(REACTOR.with(|handle| handle.future_storage.borrow_mut().insert(future)))
    }

    /// Crate a local_waker from InnerWaker.
    fn local_waker(self) -> LocalWaker {
        unsafe { local_waker(Arc::new(self)) }
    }
}

impl Wake for InnerWaker {
    /// Wake the future which InnerWaker reference to.
    fn wake(arc_self: &Arc<InnerWaker>) {
        let waker = unsafe { local_waker(arc_self.clone()) };
        let mut handle = Reactor::handle();
        let mut context = Context::new(&waker, &mut handle);
        REACTOR.with(|handle| {
            let res;
            {
                let future_obj = &mut handle.future_storage.borrow_mut()[arc_self.0];
                let future = PinMut::new(future_obj);
                res = future.poll(&mut context);
            }
            match res {
                Poll::Ready(_) => {
                    debug!("Future done");
                    handle.future_storage.borrow_mut().remove(arc_self.0);
                },
                Poll::Pending => {
                    debug!("Future not yet ready");
                }
            }
        });

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
            waker_storage: RefCell::new(Slab::new()),
            future_storage: RefCell::new(Slab::new()),
        })
    }

    /// Single iteration of event loop.
    fn iterate(&self) -> Result<(), io::Error> {
        debug!("Core iteration start");
        let mut events = self.events.borrow_mut();
        let _ready = self.poll.poll(&mut events, None)?;
        for event in &*events {
            let mio::Token(key) = event.token();
            {
                let waker = &mut self.waker_storage.borrow_mut()[key];
                waker.wake();
            }
            self.waker_storage.borrow_mut().remove(key);
        }
        debug!("Core iteration end");
        Ok(())
    }

    /// Spawn the future and do event loop.
    fn start_loop(&self) -> Result<(), io::Error> {
        while self.future_storage.borrow().len() > 0 {
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
        let token = mio::Token(self.waker_storage.borrow_mut().insert(waker));
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

    /// The real spawn function.
    fn do_spawn(&self, future: FutureObj<'static, ()>) {
        let waker = InnerWaker::new(future).local_waker();
        waker.wake(); // Just wake it to do first polling
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