fn main() {
    let tmux = murus::Tmux::try_new().expect("We must be in tmux.");

    let sessions = tmux.list_sessions().unwrap();

    println!("Got sessions:");

    for session in &sessions {
        println!("\t{session:?}")
    }
}
