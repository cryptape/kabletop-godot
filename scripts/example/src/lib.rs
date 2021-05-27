use gdnative::prelude::*;

mod classes;

// Function that registers all exposed classes to Godot
fn init(handle: InitHandle) {
    handle.add_class::<classes::Controller>();
}

// Macro that create the entry-points of the dynamic library.
godot_init!(init);