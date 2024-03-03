use std::process::Command;

pub fn call_sudo_nop() -> Result<(), String> {
    make_call("sudo loop", "sudo", &["true"].to_vec())
}

pub fn make_call(name: &str, prog: &str, args: &Vec<&str>) -> Result<(), String> {
    let output = match Command::new(prog).args(args).output() {
        Ok(output) => output,
        Err(err) => return Err(format!("command {} failed: {}", name, err)),
    };
    log_call_output(output.stdout);
    Ok(())
}

fn log_call_output(output: Vec<u8>) {
    log::trace!(
        "\"\"\"{}\"\"\"",
        std::str::from_utf8(&output)
            .or::<String>(Ok("<could not read output as utf-8>"))
            .unwrap()
    );
}
