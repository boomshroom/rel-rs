//! The `Pointer` trait allows `Rel` to behave like standard smart pointers
//! by converting them to and from a standard representation.
//! The other traits build on `Pointer` by signalling that certain bahavior
//! is allowed or disallowed.

use core::ptr::{self, NonNull};

/// The base `Pointer` trait. This is implemented for all pointer types usable with this library.
/// It plays a similar role to `Deref`, but uses raw pointers and can be used to reconstruct
/// the original pointer.
pub trait Pointer: Sized {
    /// The type this pointer is pointing at.
    type Target;

    /// Convert this smart pointer into a raw pointer
    /// for arithmetic to be performed with it.
    fn into_raw(self) -> *const Self::Target;

    /// Reconstruct the original pointer from its raw form.
    unsafe fn from_raw(p: *const Self::Target) -> Self;
}

/// An extension to `Pointer` analogous to `DerefMut`.
/// Used to indicate that this pointer can be used to write to the underlying value.
pub trait PointerMut: Pointer {
    /// A wrapper around `Pointer::into_raw` to obtain a mutible pointer.
    fn into_raw_mut(self) -> *mut Self::Target {
        self.into_raw() as *mut _
    }
    /// Reconstruct the original pointer from a mutible raw pointer.
    unsafe fn from_raw_mut(p: *mut Self::Target) -> Self {
        Self::from_raw(p as *const _)
    }
}

/// Denotes that this pointer will never be null.
pub trait PointerNonNull: Pointer {
    /// A wrapper around `Pointer::into_raw` to obtain a `NonNull` pointer.
    fn into_raw_nonnull(self) -> NonNull<Self::Target> {
        NonNull::new(self.into_raw() as *mut _).expect("Pointer claimed to be non-null was null")
    }
}

/// The oposite of `PointerNonNull`.
/// Denotes that this pointer possesses a null state where it contains nothing.
pub trait PointerNullable: Pointer {
    /// Obtain an empty instance of this pointer.
    fn get_null() -> Self {
        unsafe { Self::from_raw(ptr::null()) }
    }
}

/// Raw Pointers

impl<T> Pointer for *const T {
    type Target = T;

    fn into_raw(self) -> *const T {
        self
    }
    unsafe fn from_raw(p: *const T) -> Self {
        p
    }
}

impl<T> Pointer for *mut T {
    type Target = T;

    fn into_raw(self) -> *const T {
        self as *const T
    }
    unsafe fn from_raw(p: *const T) -> Self {
        p as *mut T
    }
}

impl<T> PointerMut for *mut T {}
impl<T> PointerNullable for *const T {}
impl<T> PointerNullable for *mut T {}

/// References

impl<'a, T> Pointer for &T {
    type Target = T;

    fn into_raw(self) -> *const T {
        self as *const T
    }
    unsafe fn from_raw(p: *const T) -> Self {
        &*p
    }
}

impl<'a, T> Pointer for &mut T {
    type Target = T;

    fn into_raw(self) -> *const T {
        self as *const T
    }
    unsafe fn from_raw(p: *const T) -> Self {
        &mut *(p as *mut T)
    }
}

impl<'a, T> PointerMut for &mut T {}

impl<'a, T> PointerNonNull for &T {
    fn into_raw_nonnull(self) -> NonNull<T> {
        self.into()
    }
}

impl<'a, T> PointerNonNull for &mut T {
    fn into_raw_nonnull(self) -> NonNull<T> {
        self.into()
    }
}

/// `NonNull`

impl<T> Pointer for NonNull<T> {
    type Target = T;

    fn into_raw(self) -> *const T {
        self.as_ptr() as *const T
    }
    unsafe fn from_raw(p: *const T) -> Self {
        Self::new_unchecked(p as *mut T)
    }
}

impl<T> PointerMut for NonNull<T> {}

impl<T> PointerNonNull for NonNull<T> {
    fn into_raw_nonnull(self) -> Self {
        self
    }
}

/// Optional pointer

impl<P: PointerNonNull> Pointer for Option<P> {
    type Target = P::Target;

    fn into_raw(self) -> *const P::Target {
        self.map_or_else(ptr::null, P::into_raw)
    }
    unsafe fn from_raw(p: *const P::Target) -> Self {
        if p.is_null() {
            None
        } else {
            Some(P::from_raw(p))
        }
    }
}

impl<P: PointerNonNull + PointerMut> PointerMut for Option<P> {}
impl<P: PointerNonNull> PointerNullable for Option<P> {
    fn get_null() -> Self {
        None
    }
}

#[cfg(all(
    any(feature = "alloc", feature = "std"),
    not(all(feature = "alloc", feature = "std"))
))]
mod _alloc {
    #[cfg(feature = "std")]
    use std as alloc;

    use super::*;

    /// Box
    use alloc::boxed::Box;

    impl<T> Pointer for Box<T> {
        type Target = T;

        fn into_raw(self) -> *const T {
            Box::into_raw(self) as *const T
        }
        unsafe fn from_raw(p: *const T) -> Self {
            Box::from_raw(p as *mut T)
        }
    }

    impl<T> PointerMut for Box<T> {}

    impl<T> PointerNonNull for Box<T> {
        #[cfg(feature = "nightly")]
        fn into_raw_nonnull(self) -> NonNull<T> {
            Box::into_raw_non_null(self)
        }
    }

    /// Rc
    use alloc::rc::Rc;

    impl<T> Pointer for Rc<T> {
        type Target = T;

        fn into_raw(self) -> *const T {
            Rc::into_raw(self)
        }
        unsafe fn from_raw(p: *const T) -> Self {
            Rc::from_raw(p)
        }
    }

    impl<T> PointerNonNull for Rc<T> {
        #[cfg(feature = "nightly")]
        fn into_raw_nonnull(self) -> NonNull<T> {
            Rc::into_raw_non_null(self)
        }
    }

    /// Arc
    use alloc::sync::Arc;

    impl<T> Pointer for Arc<T> {
        type Target = T;

        fn into_raw(self) -> *const T {
            Arc::into_raw(self)
        }
        unsafe fn from_raw(p: *const T) -> Self {
            Arc::from_raw(p)
        }
    }

    impl<T> PointerNonNull for Arc<T> {
        #[cfg(feature = "nightly")]
        fn into_raw_nonnull(self) -> NonNull<T> {
            Arc::into_raw_non_null(self)
        }
    }
}
