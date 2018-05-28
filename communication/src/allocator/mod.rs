//! Types and traits for the allocation of channels between threads, process, and computers.

pub use self::thread::Thread;
pub use self::process::Process;
pub use self::binary::Binary;
pub use self::generic::{Generic, GenericBuilder};

use bytes::arc::Bytes;
use abomonation::{Abomonation, abomonated::Abomonated, encode, measure};

pub mod thread;
pub mod process;
pub mod binary;
pub mod generic;
pub mod process_binary;

use {Data, Push, Pull};

/// Possible returned representations from a channel.
enum TypedOrBinary<T> {
    /// Binary representation.
    Binary(Abomonated<T, Bytes>),
    /// Rust typed instance.
    Typed(T),
}

pub enum RefOrMut<'a, T> where T: 'a {
    Ref(&'a T),
    Mut(&'a mut T),
}

impl<'a, T: 'a> ::std::ops::Deref for RefOrMut<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        match self {
            RefOrMut::Ref(reference) => reference,
            RefOrMut::Mut(reference) => reference,
        }
    }
}

impl<'a, T: Clone+'a> RefOrMut<'a, T> {
    /// Extracts the contents of `self`, either by cloning or swapping.
    ///
    /// This consumes `self` because its contents are now in an unknown state.
    pub fn swap<'b>(self, element: &'b mut T) {
        match self {
            RefOrMut::Ref(reference) => element.clone_from(reference),
            RefOrMut::Mut(reference) => ::std::mem::swap(reference, element),
        };
    }
    /// Extracts the contents of `self`, either by cloning or swapping.
    ///
    /// This consumes `self` because its contents are now in an unknown state.
    pub fn replace(self, mut element: T) -> T {
        self.swap(&mut element);
        element
    }
}

pub struct Message<T> {
    payload: TypedOrBinary<T>,
}

impl<T> Message<T> {
    pub fn from_typed(typed: T) -> Self {
        Message { payload: TypedOrBinary::Typed(typed) }
    }
    pub fn if_typed(self) -> Option<T> {
        match self.payload {
            TypedOrBinary::Binary(_) => None,
            TypedOrBinary::Typed(typed) => Some(typed),
        }
    }
    pub fn if_mut(&mut self) -> Option<&mut T> {
        match &mut self.payload {
            TypedOrBinary::Binary(_) => None,
            TypedOrBinary::Typed(typed) => Some(typed),
        }
    }
}

impl<T: Abomonation> Message<T> {
    pub fn from_bytes(bytes: Bytes) -> Self {

        unsafe {
            let abomonated = Abomonated::new(bytes).expect("Abomonated::new() failed.");
            Message { payload: TypedOrBinary::Binary(abomonated) }
        }

    }

    pub fn as_ref_or_mut(&mut self) -> RefOrMut<T> {
        match &mut self.payload {
            TypedOrBinary::Binary(bytes) => { RefOrMut::Ref(bytes) },
            TypedOrBinary::Typed(typed) => { RefOrMut::Mut(typed) },
        }
    }

    fn length_in_bytes(&self) -> usize {
        match &self.payload {
            TypedOrBinary::Binary(bytes) => { bytes.as_bytes().len() },
            TypedOrBinary::Typed(typed) => { measure(typed) },
        }
    }
    fn into_bytes<W: ::std::io::Write>(&self, writer: &mut W) {
        match &self.payload {
            TypedOrBinary::Binary(bytes) => {
                writer.write_all(bytes.as_bytes()).expect("Message::into_bytes(): write_all failed.");
            },
            TypedOrBinary::Typed(typed) => {
                unsafe { encode(typed, writer).expect("Message::into_bytes(): Abomonation::encode failed"); }
            },
        }
    }
}

impl<T> ::std::ops::Deref for Message<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        // TODO: In principle we have aready decoded, but let's go again
        match &self.payload {
            TypedOrBinary::Binary(bytes) => { bytes },
            TypedOrBinary::Typed(typed) => { typed },
        }
    }
}

impl<T: Abomonation+Clone> Message<T> {
    /// Produces a typed instance of the wrapped element.
    pub fn into_typed(self) -> T {
        match self.payload {
            TypedOrBinary::Binary(bytes) => bytes.clone(),
            TypedOrBinary::Typed(instance) => instance,
        }
    }

    pub fn as_mut(&mut self) -> &mut T {
        let mut decoded = None;
        if let TypedOrBinary::Binary(bytes) = &mut self.payload {
            decoded = Some(bytes.clone());
        }
        if let Some(decoded) = decoded {
            self.payload = TypedOrBinary::Typed(decoded);
        }
        if let TypedOrBinary::Typed(typed) = &mut self.payload {
            typed
        }
        else {
            unreachable!()
        }
    }

}

// The Communicator trait presents the interface a worker has to the outside world.
// The worker can see its index, the total number of peers, and acquire channels to and from the other workers.
// There is an assumption that each worker performs the same channel allocation logic; things go wrong otherwise.
pub trait Allocate {
    /// The index of the worker out of `(0..self.peers())`.
    fn index(&self) -> usize;
    /// The number of workers.
    fn peers(&self) -> usize;
    /// Constructs several send endpoints and one receive endpoint.
    // fn allocate<T: Data>(&mut self) -> (Vec<Box<Push<T>>>, Box<Pull<T>>, Option<usize>);
    fn allocate<T: Data>(&mut self) -> (Vec<Box<Push<Message<T>>>>, Box<Pull<Message<T>>>, Option<usize>);

    fn pre_work(&mut self) { }
    fn post_work(&mut self) { }
}
