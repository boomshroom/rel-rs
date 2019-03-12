#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "alloc", feature(alloc))]
#![cfg_attr(
    feature = "nightly",
    feature(
        ptr_wrapping_offset_from,
        box_into_raw_non_null,
        rc_into_raw_non_null,
    )
)]

#![deny(missing_docs)]

//! A small library focusing on the `Rel` struct.
//! `Rel` is a relative pointer that uses a provided integer type to use as an offset.
//! In addition, it can be configured to masquerade as one of the pointer types in std.
//! See `RelRef` and `RelBox` for examples on wrapping

#[cfg(all(feature = "std", feature = "alloc"))]
compile_error!("Please select only 1 of std or alloc.");

#[cfg(feature = "alloc")]
extern crate alloc;

use core::marker::{PhantomData, PhantomPinned};
use core::ops::{Deref, DerefMut};
use num_traits::PrimInt;

pub mod traits;

use traits::{Pointer, PointerMut, PointerNonNull, PointerNullable};

/// The core of this crate. Rel wraps an existing smart (or raw)
/// pointer and store the offset to its target rather than the
/// absolute address. This type does not implement Unpin as moving
/// the value also moves the destination of the pointer.
/// It is safe to move this if it's pointing into the same block
/// of memory as the one its target is located in.
pub struct Rel<P: Pointer, I: PrimInt=isize> {
    offset: I,
    _pd: PhantomData<P>,
    _pp: PhantomPinned,
}

/// A relative immutible reference. Does not own its contents.
pub type RelRef<'a, T, I=isize> = Rel<&'a T, I>;

/// A relative mutible reference. Possesses the same permissions
/// as a mutible reference.
pub type RelMut<'a, T, I=isize> = Rel<&'a mut T, I>;

#[cfg(any(feature = "alloc", feature = "std"))]
/// A relative owned pointer. Behaves like a `Box`
/// when determining ownership.
pub type RelBox<T, I=isize> = Rel<Box<T>, I>;

mod err {
    #[cfg(feature = "std")]
    use core::fmt::Debug;
    use core::fmt::{self, Display, Formatter, UpperHex};
    use core::marker::PhantomData;
    use num_traits::Bounded;

    /// The error returned when the target address is too far to fit
    /// in the provided offset type.
    #[derive(Debug, Copy, Clone)]
    pub struct OutOfRange<I>(isize, PhantomData<I>);

    impl<I> OutOfRange<I> {
        pub(crate) const fn new(offset: isize) -> Self {
            Self(offset, PhantomData)
        }
    }

    impl<I: Bounded + UpperHex> Display for OutOfRange<I> {
        fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
            write!(
                f,
                "Offset of {:#X} outside of range ({:#X}, {:#X})",
                self.0,
                I::min_value(),
                I::max_value()
            )
        }
    }

    #[cfg(feature = "std")]
    impl<I: Bounded + UpperHex + Debug> std::error::Error for OutOfRange<I> {}
}

pub use err::OutOfRange;

impl<P: Pointer, I: PrimInt> Rel<P, I> {
    /// Initializes the relative pointer with a provided pointer.
    /// Assumes that the relative pointer is not yet initialized
    /// and will leak if it already contains a value.
    pub unsafe fn set_raw(this: *mut Self, p: P) -> Result<(), OutOfRange<I>> {
        let p = p.into_raw();
        let offset = Self::offset_to(this, p)?;
        *this = Self {
            offset,
            _pd: PhantomData,
            _pp: PhantomPinned,
        };
        Ok(())
    }

    /// Replaces the target of this pointer with the provided one.
    /// Returns the previous smart pointer as well as whether
    /// or not the reinitialization failed.
    pub fn replace(&mut self, p: P) -> (P, Result<(), OutOfRange<I>>) {
        let inner = unsafe { P::from_raw(self.get_raw()) };
        (inner, unsafe { Self::set_raw(self, p) })
    }

    fn offset_to(this: *const Self, target: *const P::Target) -> Result<I, OutOfRange<I>> {
        let from = this as *const u8;
        let to = target as *const u8;

        let offset = {
            #[cfg(feature = "nightly")]
            {
                to.wrapping_offset_from(from)
            }
            #[cfg(not(feature = "nightly"))]
            {
                (to as isize).wrapping_sub(from as isize)
            }
        };
        I::from(offset).ok_or_else(|| OutOfRange::new(offset))
    }

    /// Acquire a raw pointer to the target
    /// This can be passed to `Pointer::from_raw`
    /// to reconstruct the original smart pointer.
    pub fn get_raw(&self) -> *const P::Target {
        let offset = self.offset.to_isize().unwrap();
        (self as *const _ as *const u8).wrapping_offset(offset) as *const P::Target
    }

    fn with_inner<T>(&self, f: impl FnOnce(&P) -> T) -> T {
        let inner = unsafe { P::from_raw(self.get_raw()) };
        let res = f(&inner);
        core::mem::forget(inner);
        res
    }
}

impl<P: PointerMut, I: PrimInt> Rel<P, I> {
    /// Acquire a raw mutible pointer to the target
    /// This can be passed to `PointerMut::from_raw`
    /// to reconstruct the original smart pointer.
    pub fn get_raw_mut(&mut self) -> *mut P::Target {
        self.get_raw() as *mut _
    }
}

impl<P: Pointer + Clone, I: PrimInt> Rel<P, I> {
    /// Clones the value in this pointer into `target`.
    /// May or may not clone the target value or just the pointer
    /// depending on which type this was initialized as.
    pub unsafe fn clone_into_raw(&self, target: *mut Self) -> Result<(), OutOfRange<I>> {
        let p = self.with_inner(Clone::clone);
        Self::set_raw(target, p)
    }

    /// `Rel::clone_into_raw` with the arguments flipped.
    /// This is to more closely match `Clone::clone_from`.
    pub unsafe fn clone_from_raw(this: *mut Self, source: &Self) -> Result<(), OutOfRange<I>> {
        source.clone_into_raw(this)
    }
}

impl<P: Pointer, I: PrimInt> Drop for Rel<P, I> {
    fn drop(&mut self) {
        unsafe { P::from_raw(self.get_raw()) };
    }
}

impl<P: PointerNullable, I: PrimInt> Default for Rel<P, I> {
    fn default() -> Self {
        Self::new()
    }
}

impl<P: PointerNullable, I: PrimInt> Rel<P, I> {
    /// Initialize an empty instance of this pointer.
    /// Only allowed if the underlying pointer type supports a
    /// null value;
    pub fn new() -> Self {
        Self {
            offset: I::zero(),
            _pd: PhantomData,
            _pp: PhantomPinned,
        }
    }

    /// Retrieve the smart pointer and replace it with a
    /// null. Analagous to `Option::take`.
    pub fn take(&mut self) -> P {
        let inner = unsafe { P::from_raw(self.get_raw()) };
        *self = Self::new();
        inner
    }
}

impl<P: PointerNullable + Clone, I: PrimInt> Rel<P, I> {
    /// Reinitializes `target` with the value in `self`.
    /// Safe wrapper around `Rel::clone_into_raw` that `Drop`s
    /// the old value in `target`.
    pub fn clone_into(&self, target: &mut Self) -> Result<(), OutOfRange<I>> {
        target.take();
        unsafe { Self::clone_into_raw(self, target) }
    }

    /// `Rel::clone_into` with the arguments flipped.
    /// This is to more closely match `Clone::clone_from`.
    pub fn clone_from(&mut self, source: &Self) -> Result<(), OutOfRange<I>> {
        source.clone_into(self)
    }
}

impl<P: PointerNonNull + Deref<Target = <P as Pointer>::Target>, I: PrimInt> Deref for Rel<P, I> {
    type Target = <P as Pointer>::Target;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.get_raw() }
    }
}

impl<P: PointerNonNull + PointerMut + DerefMut<Target = <P as Pointer>::Target>, I: PrimInt>
    DerefMut for Rel<P, I>
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.get_raw_mut() }
    }
}
