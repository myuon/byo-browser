use std::process::{Child, Command};

pub struct DroppableProcess {
    child: Child,
}

impl DroppableProcess {
    pub fn new(command: &mut Command) -> Result<Self, Box<dyn std::error::Error>> {
        let child = command.spawn()?;
        Ok(Self { child })
    }
}

impl Drop for DroppableProcess {
    fn drop(&mut self) {
        println!("Killing child process");

        if let Err(err) = self.child.kill() {
            eprintln!("Failed to kill child process: {}", err);
        }
    }
}
