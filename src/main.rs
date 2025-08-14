use bitcoincore_rpc::Auth;
use clap::Parser;
use tracker::{Config, start};

#[derive(Parser)]
struct App {
    #[clap(long, short = 'r', default_value = "127.0.0.1:18443")]
    rpc: String,
    #[clap(short = 'a', long, default_value = "username:password")]
    auth: String,
    #[clap(short = 's', long, default_value = "127.0.0.1:8080")]
    address: String,
    #[clap(short = 'c', long, default_value = "9051")]
    control_port: u16,
    #[clap(long, default_value = "")]
    tor_auth_password: String,
    #[clap(long, default_value = "9050")]
    socks_port: u16,
    #[clap(long, default_value = ".tracker")]
    datadir: String,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let args = App::parse();

    let (user, pass) = {
        let parts: Vec<_> = args.auth.split(':').collect();
        (parts[0].to_string(), parts[1].to_string())
    };

    #[cfg(not(feature = "integration-test"))]
    let cfg = Config {
        rpc_url: args.rpc,
        rpc_auth: Auth::UserPass(user, pass),
        address: args.address,
        control_port: args.control_port,
        tor_auth_password: args.tor_auth_password,
        socks_port: args.socks_port,
        datadir: args.datadir,
    };

    #[cfg(feature = "integration-test")]
    let cfg = Config {
        rpc_url: args.rpc,
        rpc_auth: Auth::UserPass(user, pass),
        address: args.address,
        datadir: args.datadir,
    };

    start(cfg).await;
}
