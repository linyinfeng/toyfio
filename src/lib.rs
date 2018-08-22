#![feature(futures_api, pin, async_await, await_macro, arbitrary_self_types)]

use std::{io, ops::Deref, sync::Arc, rc::Rc, cell::RefCell, mem::PinMut, thread_local};
use futures::{
    Poll,
    future::{Future, FutureObj},
    task::{Context, Spawn, SpawnObjError, Wake, LocalWaker, local_waker}
};
use log::{debug, trace};

mod leak_storage;
use crate::leak_storage::LeakStorage;

// Re-export modules exports
mod async_tcp_stream;
pub use crate::async_tcp_stream::AsyncTcpStream;
mod async_tcp_listener;
pub use crate::async_tcp_listener::AsyncTcpListener;

thread_local! {
    /// The global reactor.
    static REACTOR: Handle = Reactor::new(1024).expect("failed to initialize thread local reactor");
}

/// Reactor for futures.
#[derive(Debug)]
pub struct Reactor {
    poll: mio::Poll,
    events: RefCell<mio::Events>,
    counter: RefCell<usize>,
}

/// Handle of `Reactor`.
///
/// Can be deref to a `Reactor`.
#[derive(Debug, Clone)]
pub struct Handle(Rc<Reactor>);

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
        InnerWaker(LeakStorage::insert(future))
    }

    /// Crate a local_waker from InnerWaker.
    fn local_waker(self) -> LocalWaker {
        unsafe { local_waker(Arc::new(self)) }
    }
}

impl Wake for InnerWaker {
    /// Wake the future which InnerWaker reference to.
    fn wake(arc_self: &Arc<InnerWaker>) {
        let mut handle = Reactor::handle();
        let waker = unsafe { local_waker(arc_self.clone()) };
        let mut context = Context::new(&waker, &mut handle);
        let future = PinMut::new(unsafe { LeakStorage::<FutureObj<'static, ()>>::get_ref_mut(arc_self.0) });
        match future.poll(&mut context) {
            Poll::Ready(_) => {
                debug!("future done");
                unsafe { LeakStorage::<FutureObj<'static, ()>>::remove(arc_self.0) };
                REACTOR.with(|handle| {
                    handle.report_finished();
                });
            },
            Poll::Pending => debug!("future not yet ready"),
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
        debug!("core iteration start");
        let mut events = self.events.borrow_mut();
        let _ready = self.poll.poll(&mut events, None)?;
        for event in &*events {
            let mio::Token(key) = event.token();
            let waker = unsafe { LeakStorage::<LocalWaker>::get(key) };
            waker.wake();
        }
        debug!("core iteration end");
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
    pub fn register<E>(&self, handle: &E) -> Result<(), io::Error>
    where E: mio::Evented + ?Sized {
        // Use Token(0) to just hold the place
        trace!("register called for {:p}", handle);
        self.poll.register(handle, mio::Token(0), mio::Ready::empty(), mio::PollOpt::oneshot())?;
        Ok(())
    }

    /// Manipulate waker and interest.
    pub fn reregister<E>(&self, handle: &E, waker: LocalWaker, interest: mio::Ready) -> Result<(), io::Error>
        where E: mio::Evented + ?Sized {
        trace!("reregister called for {:p}, interest: {:?}", handle, interest);
        let token = mio::Token(LeakStorage::insert(waker));
        self.poll.reregister(handle, token, interest, mio::PollOpt::oneshot())?;
        Ok(())
    }

    fn report_new(&self) {
        *self.counter.borrow_mut() += 1;
    }

    fn report_finished(&self) {
        if *self.counter.borrow() == 0 {
            panic!("reactor counter lower than 0");
        } else {
            *self.counter.borrow_mut() -= 1;
        }
    }

    /// The real spawn function.
    fn do_spawn(&self, future: FutureObj<'static, ()>) {
        trace!("spawn future: {:?}", future);
        self.report_new();
        let inner_waker = InnerWaker::new(future);
        let waker = inner_waker.local_waker();
        waker.wake();
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
