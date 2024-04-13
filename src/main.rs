mod image_container;
mod locker;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    match pwd::Passwd::current_user() {
        Some(current_user) => match current_user.name.as_str() {
            "greeter" => locker::main(),
            _ => locker::main(),
        },
        _ => Err("failed to determine current user".into()),
    }
}
