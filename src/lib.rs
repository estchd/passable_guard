//! # Passable Guard
//!
//! The Passable Guard Crate provides a way to check for FFI memory leaks at runtime.
//!
//! This is achieved by providing a [PassableContainer] class that encapsulates
//! a [Passable] Object that can be converted to a raw pointer to pass it over a FFI boundary.
//!
//! This [PassableContainer] combines the raw pointer with a [PassableGuard]
//! when converting the [Passable].
//!
//! This [PassableGuard] will panic if it is dropped before recombining it with the raw pointer.
//!
//! That way, you will at least get a panic instead of leaking memory
//!
//! ## Example
//!
//! For this example, we will create a CString and pass it to a fictional FFI function `setName`,
//! using a [PassableContainer] to guard against Memory Leaks
//!
//! ``` rust
//! use std::ffi::CString;
//! use passable_guard::PassableContainer;
//!
//! extern "C" {
//!     /// Takes a pointer to a NULL-Terminated utf-8 string
//!     /// Returns 0 on failure and >0 on success
//!     fn setName(ptr: *mut u8) -> u8;
//! }
//!
//! fn passable_example(name: CString) -> Result<(),()> {
//!     let passable = PassableContainer::new(name); // Create the Container from the name CString
//!
//!     let (guard, ptr) = passable.pass(); // Convert the Container into a raw pointer (and get the guard for it as well)
//!
//!     let result = unsafe {
//!          setName(ptr) // Call the FFI function and give it our pointer
//!     };
//!
//!     unsafe {
//!         // Reconstitute the Guard and Pointer back into a Container
//!         // The pointers will be the same since we use the pointer we got from the pass method
//!         // This might cause UB if setName modifies the Memory
//!         guard.reconstitute(ptr).unwrap();
//!     }
//!     drop(ptr); // Drop the Pointer so we do not use it again
//!
//!     return if result == 0 {
//!         Err(())
//!     }
//!     else {
//!         Ok(())
//!     }
//! }
//! ```
//!
//! Let's look at an example that Panics
//!
//! ``` rust
//! use std::ffi::CString;
//! use passable_guard::PassableContainer;
//!
//! extern "C" {
//!     /// Takes a pointer to a NULL-Terminated utf-8 string
//!     /// Returns 0 on failure and >0 on success
//!     fn setName(ptr: *mut u8) -> u8;
//! }
//!
//! fn passable_example(name: CString) -> Result<(),()> {
//!     let passable = PassableContainer::new(name); // Create the Container from the name CString
//!
//!     let (guard, ptr) = passable.pass(); // Convert the Container into a raw pointer (and get the guard for it as well)
//!
//!     let result = unsafe {
//!          setName(ptr) // Call the FFI function and give it our pointer
//!     };
//!
//!     // Drop the Pointer so we do not use it again
//!     // This means that we cannot possibly reconstitute the Guard and pointer
//!     drop(ptr);
//!
//!     return if result == 0 {
//!         Err(())
//!     }
//!     else {
//!         Ok(())
//!     }
//!     // The Function will panic here since the Guard has been dropped without being reconstituted
//!     // Without the Guard, we would have now subtly leaked the String Memory
//! }
//! ```

use std::marker::PhantomData;
use std::ffi::CString;

/// An Error that can occur while reconstituting a [Passable] from a pointer
#[derive(Debug, Clone)]
pub enum ReconstituteError<PTR, PAS: Passable<PTR>> {
    PointerMismatch{passed: *mut PTR, reconstituted: *mut PTR},
    ReconstituteError{error: PAS::ReconstituteError}
}

/// A Container that allows for checked passing of a pointer over a FFI boundary
#[derive(Debug, Clone)]
pub struct PassableContainer<PTR, PAS: Passable<PTR>> {
    value: PAS,
    _phantom: PhantomData<PTR>
}

impl<PTR, PAS: Passable<PTR>> PassableContainer<PTR, PAS> {
    /// Creates a new [PassableContainer] from a [Passable]
    pub fn new(passable: PAS) -> Self {
        Self {
            value: passable,
            _phantom: Default::default()
        }
    }

    /// Get back the [Passable] from this Container
    pub fn into_inner(self) -> PAS {
        self.value
    }

    /// Convert the [PassableContainer] into a pointer to pass it over a FFI boundary
    pub fn pass(self) -> (PassableGuard<PTR, PAS>, *mut PTR) {
        let ptr = self.value.pass();
        let guard = PassableGuard {
            ptr,
            _phantom: Default::default()
        };
        (guard, ptr)
    }

    /// Convert the [PassableContainer] into a pointer to pass if over a FFI Boundary
    ///
    /// ### Unsafe
    /// Since this does not create a [PassableGuard] to accompany the pointer, it is unsafe
    pub unsafe fn pass_unguarded(self) -> *mut PTR {
        self.value.pass()
    }
}

/// A guard for a [PassableContainer] that has been converted into a pointer to be passed over a FFI boundary
///
/// ### Panic
/// If this guard is dropped before it has been reconstituted with the original pointer, it will panic
#[derive(Debug, Clone)]
pub struct PassableGuard<PTR, PAS: Passable<PTR>> {
    ptr: *mut PTR,
    _phantom: PhantomData<PAS>
}

impl<PTR, PAS: Passable<PTR>> PassableGuard<PTR, PAS> {
    /// Reconstitute a raw pointer back into a [PassableContainer]
    ///
    /// ### Errors
    /// Will return an Error if the pointer points do a different memory address then the pointer that was originally created by the pass method of the Container
    /// Will return an Error if the memory was modified by the FFI
    ///
    /// ### Unsafe
    /// This function is unsafe because if the memory was modified by the FFI, it can cause UB when trying to reconstitute the [Passable]
    ///
    /// As an example, a FFI removing the terminating NULL from a NULL-terminated C-String, it can cause reads outside the original Buffer
    ///
    /// Additionally, continuing to use the pointer after the [PassableContainer] will lead to UB
    pub	unsafe fn reconstitute(self, ptr: *mut PTR) -> Result<PassableContainer<PTR, PAS>, ReconstituteError<PTR, PAS>> {
        if self.ptr != ptr {
            return Err(
                ReconstituteError::PointerMismatch {
                    passed: self.ptr,
                    reconstituted: ptr
                }
            );
        }

        PAS::reconstitute(ptr)
            .map(|passable| PassableContainer::new(passable))
            .map_err(
                |err|
                    ReconstituteError::ReconstituteError {error: err}
            )
    }
}

impl<PTR, PAS: Passable<PTR>> Drop for PassableGuard<PTR, PAS> {
    /// This function will always panic because it should never be called outside of Error States
    fn drop(&mut self) {
        panic!("Passable Guard dropped before being reconstituted");
    }
}

/// Base Trait for something that can be converted into a raw pointer to its underlying buffer to pass it over a FFI boundary
pub trait Passable<PTR> : Sized {
    type ReconstituteError;

    /// Convert the [Passable] into a raw pointer to its underlying data
    ///
    /// ### Notes
    /// Implementations must take care to ensure the underlying memory is not freed in this conversion
    /// It must also be ensured that the memory stays valid until the pointer is reconstituted
    fn pass(self) -> *mut PTR;

    /// Reconstitute the [Passable] from a raw pointer crated by the pass method
    ///
    /// ### Notes
    /// Implementations should try to handle modification of the data by the FFI but no guarantees can be made about this
    ///
    /// ### Unsafe
    /// Although Implementations should try to handle data modification by the FFI, there are modifications the cannot be detected when trying to reconstitute.
    /// This includes freeing the memory by the FFI, removing the trailing NULL of a NULL-Terminated string and similar modifications.
    unsafe fn reconstitute(ptr: *mut PTR) -> Result<Self, Self::ReconstituteError>;
}

impl Passable<u8> for CString {
    type ReconstituteError = ();

    fn pass(self) -> *mut u8 {
        self.into_raw() as *mut u8
    }

    unsafe fn reconstitute(ptr: *mut u8) -> Result<Self, Self::ReconstituteError> {
        Ok(CString::from_raw(ptr as *mut i8))
    }
}
