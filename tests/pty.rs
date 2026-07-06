#![cfg(unix)]

use terminal_svg::term;
use terminal_svg::term::screen::PenColor;

#[test]
fn pty_capture_renders_color() {
    let command = vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        r"printf '\033[31mhi\033[0m pty\n'".to_string(),
    ];
    let bytes = terminal_svg::capture::pty::run(&command, 80, 24, Some(10)).unwrap();

    let screen = term::interpret(&bytes, 80, 24);

    let red_run = screen.rows[0]
        .iter()
        .find(|run| run.text == "hi")
        .expect("red run captured through the pty");
    assert_eq!(red_run.fg, PenColor::Indexed(1));
}

#[test]
fn pty_timeout_kills_command() {
    let command = vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        "echo started; sleep 30".to_string(),
    ];
    let start = std::time::Instant::now();
    let bytes = terminal_svg::capture::pty::run(&command, 80, 24, Some(1)).unwrap();
    assert!(start.elapsed().as_secs() < 5, "timeout was not enforced");
    assert!(String::from_utf8_lossy(&bytes).contains("started"));
}
