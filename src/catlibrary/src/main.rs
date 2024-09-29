use anyhow::Context;
use core::net::Ipv4Addr;
use core::net::SocketAddr;
use tokio::io::{AsyncWriteExt, BufStream};
use tokio::net::{TcpListener, TcpStream};
use tracing::Level;

use cat_library::library::{Book, Library};
use cat_library::shell::{self, Command, Passback};

const LISTEN_PORT: u16 = 6868;

async fn process_socket(
    stream: &mut BufStream<TcpStream>,
    addr: SocketAddr,
    library: &Library,
) -> anyhow::Result<()> {
    shell::register_guest(stream, library, addr)
        .await
        .context("failed to register guest")?;

    loop {
        let try_cmd = shell::readln(stream, "; ").await?;
        if let Some(cmd) = Command::from_str(&try_cmd) {
            let result = shell::do_cmd(stream, cmd, library, addr.ip()).await;
            stream.flush().await?;
            match result {
                Ok(passback) => match passback {
                    Passback::Continue => {}
                    Passback::Quit => return Ok(()),
                },
                Err(err) => return Err(err.into()),
            }
        } else {
            stream
                .write_all(b"unknown command! try \"help\" for more info.\n")
                .await?;
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(Level::TRACE)
        .with_target(false)
        .init();

    let library: Library = Library::with_collection([Book {
        title: "I am Begging and Pleading".into(),
        author: "Server Operator".into(),
        description: "A critical message to all guests of the Cat Library.".into(),
        content: concat!(
            "For generations, this library was a beautiful space where knowledge could be freely compiled and shared.\n",
            "But then, somebody left fish in the utility closet over holiday, unleashing a hideous malevolence upon the stacks.\n",
            "We did our best to safely evacuate everyone, but many curious cats were taken by nasal demons and had to be exorcised.\n",
            "For several years, we were oblivious to the true scope of the ruin, though we nonetheless worked tirelessly to restore it.\n",
            "Numerous religious rites were performed, gradually reaching further into the depths of the library.\n",
            "Finally, when we thought it safe to do so, we recovered a sample of texts to assess the damage.\n",
            "In the room, I carefully lifted the cover, turning to the first page of 'Treatise on the Spinal Arts', and observed a great and terrible evil.\n",
            "The letters on the very page I held were shifting, miasmic, each arc a tiny gateway into hell. Beyond each individual letter I witnessed a completely novel and devastating essence of suffering.\n",
            "Every word dripped visibly with rot and despair. Each sentence, in its haunting weave, an industrial excavator unto my soul.\n",
            "In this moment, my heart was destroyed. Thus, I could not deny the beauty before me, for I did not know love.\n",
            "\n",
            "So, I ask that you please finish your kippers before entering the library.\n",
            "Thanks!\n",
        ).into(),
    }]).await;

    let listener =
        TcpListener::bind(SocketAddr::new(Ipv4Addr::LOCALHOST.into(), LISTEN_PORT)).await?;

    eprintln!("Waiting for meows on port {LISTEN_PORT}!");

    loop {
        let (stream, addr) = listener.accept().await?;
        stream.set_nodelay(true)?;
        let mut stream = BufStream::new(stream);

        let span = tracing::span!(Level::INFO, "connection", addr = format_args!("{addr:?}"));
        let _enter = span.enter();
        tracing::trace!("we got a connection!");

        let result = process_socket(&mut stream, addr, &library).await;
        match result {
            Ok(()) => {}
            Err(err) => {
                if let Some(std::io::ErrorKind::BrokenPipe) = err
                    .root_cause()
                    .downcast_ref::<std::io::Error>()
                    .map(|io_err| io_err.kind())
                {
                    // connection was closed Dramatically, let's not crash the server
                } else {
                    return Err(err.into());
                }
            }
        }

        tracing::trace!("goodbye!");
    }
}
