# Passable Guard

The Passable Guard Crate provides a way to check for FFI memory leaks at runtime.

This is achieved by providing a [PassableContainer] class that encapsulates
a [Passable] Object that can be converted to a raw pointer to pass it over a FFI boundary.

This [PassableContainer] combines the raw pointer with a [PassableGuard]
when converting the [Passable].

This [PassableGuard] will panic if it is dropped before recombining it with the raw pointer.

That way, you will at least get a panic instead of leaking memory

## Example

For this example, we will create a CString and pass it to a fictional FFI function `setName`,
using a [PassableContainer] to guard against Memory Leaks

```
use std::ffi::CString;
use passable_guard::PassableContainer;

extern "C" {
    /// Takes a pointer to a NULL-Terminated utf-8 string
    /// Returns 0 on failure and >0 on success
    fn setName(ptr: *mut u8) -> u8;
}

fn passable_example(name: CString) -> Result<(),()> {
    let passable = PassableContainer::new(name); // Create the Container from the name CString

    let (guard, ptr) = passable.pass(); // Convert the Container into a raw pointer (and get the guard for it as well)

    let result = unsafe {
         setName(ptr) // Call the FFI function and give it our pointer
    };

    unsafe {
        // Reconstitute the Guard and Pointer back into a Container
        // The pointers will be the same since we use the pointer we got from the pass method
        // This might cause UB if setName modifies the Memory
        guard.reconstitute(ptr).unwrap();
    }
    drop(ptr); // Drop the Pointer so we do not use it again

    return if result == 0 {
        Err(())
    }
    else {
        Ok(())
    }
}
```

Let's look at an example that Panics

```
use std::ffi::CString;
use passable_guard::PassableContainer;

extern "C" {
    /// Takes a pointer to a NULL-Terminated utf-8 string
    /// Returns 0 on failure and >0 on success
    fn setName(ptr: *mut u8) -> u8;
}

fn passable_example(name: CString) -> Result<(),()> {
    let passable = PassableContainer::new(name); // Create the Container from the name CString

    let (guard, ptr) = passable.pass(); // Convert the Container into a raw pointer (and get the guard for it as well)

    let result = unsafe {
         setName(ptr) // Call the FFI function and give it our pointer
    };

    // Drop the Pointer so we do not use it again
    // This means that we cannot possibly reconstitute the Guard and pointer
    drop(ptr);

    return if result == 0 {
        Err(())
    }
    else {
        Ok(())
    }
    // The Function will panic here since the Guard has been dropped without being reconstituted
    // Without the Guard, we would have now subtly leaked the String Memory
}
```