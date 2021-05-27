
use gdnative::prelude::*;
use gdnative::api::*;

#[derive(NativeClass)]
#[inherit(Button)]
pub struct Controller {
    use_debug: bool
}

#[gdnative::methods]
impl Controller {
    fn new(_owner: &Button) -> Self {
        Controller {
            use_debug: false
        }
    }

    #[export]
    fn _ready(&mut self, owner: TRef<Button>) {
        godot_print!("hello world");
    }
}