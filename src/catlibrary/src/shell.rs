use core::net::{IpAddr, SocketAddr};
use core::num::{IntErrorKind, ParseIntError};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt};
use tracing::Level;

use crate::editor::{self, Editor};
use crate::library::{Book, BookID, Library, Metadata, RegisterError, UpdateEntryError};

pub enum Passback {
    Continue,
    Quit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    None,
    Help,
    Quit,
    Search,
    CheckOut,
    CheckIn,
    Read,
    Add,
    Meow,
}

impl Command {
    pub const ALL: &'static [Self] = &[
        Self::Help,
        Self::Quit,
        Self::Search,
        Self::CheckOut,
        Self::CheckIn,
        Self::Read,
        Self::Add,
    ];

    pub const fn short(self) -> &'static str {
        match self {
            Self::None => self.long(),
            Self::Help => "h",
            Self::Quit => "q",
            Self::Search => "s",
            Self::CheckOut => "co",
            Self::CheckIn => "ci",
            Self::Read => "r",
            Self::Add => "a",
            Self::Meow => self.long(),
        }
    }

    pub const fn long(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Help => "help",
            Self::Quit => "quit",
            Self::Search => "search",
            Self::CheckOut => "checkout",
            Self::CheckIn => "checkin",
            Self::Read => "read",
            Self::Add => "add",
            Self::Meow => "meow",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        if s.contains("meow") {
            return Some(Self::Meow);
        }
        if s.is_empty() {
            return Some(Self::None);
        }
        for &test in Self::ALL {
            if [test.short(), test.long()].contains(&s) {
                return Some(test);
            }
        }
        None
    }
}

pub async fn move_cursor_prev<S: AsyncRead + AsyncBufReadExt + AsyncWrite + Unpin>(
    stream: &mut S,
) -> anyhow::Result<()> {
    stream.write_all(b"\x1B[F").await?;
    Ok(())
}

pub async fn clear_line<S: AsyncRead + AsyncBufReadExt + AsyncWrite + Unpin>(
    stream: &mut S,
) -> anyhow::Result<()> {
    stream.write_all(b"\x1B[2K").await?;
    Ok(())
}

pub async fn readln<S: AsyncRead + AsyncBufReadExt + AsyncWrite + Unpin>(
    stream: &mut S,
    prompt: &str,
) -> anyhow::Result<String> {
    let mut buf = String::new();
    stream.write_all(prompt.as_bytes()).await?;
    stream.flush().await?;
    match stream.read_line(&mut buf).await {
        Ok(_n) => {
            // XXX: reallocation here is silly (bad)
            let trimmed = buf.trim().to_string();
            Ok(trimmed)
        }

        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
            todo!();
        }
        Err(e) => Err(e.into()),
    }
}

pub async fn register_guest<S: AsyncRead + AsyncBufReadExt + AsyncWrite + Unpin>(
    stream: &mut S,
    library: &Library,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    let span = tracing::span!(Level::INFO, "register_guest", addr = format_args!("{addr}"));
    let _enter = span.enter();

    let nick: Option<Arc<str>> = library.lookup_guest_by_addr(addr.ip()).await;

    if let Some(nick) = nick {
        tracing::info!(nick = &*nick, "welcome back");
        stream
            .write_all(b"Welcome back to the Cat Library!\n")
            .await?;
        stream.write_all(b"Your nickname is '").await?;
        stream.write_all(nick.as_bytes()).await?;
        stream.write_all(b"'.\n").await?;
    } else {
        stream.write_all(b"Welcome to the Cat Library!\n").await?;
        for line in [
            "this appears to be your first visit...",
            "you will need to provide a nickname.",
            "nicknames are public so that addresses can be private.",
        ] {
            stream.write_all(line.as_bytes()).await?;
            stream.write_all(b"\n").await?;
        }

        loop {
            let nick = readln(stream, "what is it? ").await?;
            if nick.is_empty() {
                continue;
            }
            match library.register_guest(addr.ip(), nick).await {
                Ok(nick) => {
                    tracing::info!(nick = &*nick, "registered new guest");
                    break;
                }
                Err(err) => match err {
                    RegisterError::AlreadyRegistered => break,
                    RegisterError::NicknameTaken => {
                        stream.write_all(b"nickname is already taken").await?;
                    }
                },
            }
        }
    }

    Ok(())
}

pub async fn enumerate_entries<S: AsyncRead + AsyncBufReadExt + AsyncWrite + Unpin>(
    stream: &mut S,
    library: &Library,
    entries: impl ExactSizeIterator<Item = (f64, BookID, Metadata)>,
) -> anyhow::Result<()> {
    for (idx, (_sim, book_id, meta)) in entries.enumerate() {
        let rank = idx + 1;
        let book = library.lookup_book_by_id(book_id).await;
        let presence = if meta.is_free() { "[in] " } else { "[out]" };
        stream
            .write_all(
                format!("{rank}. {presence} '{}', by {}.\n", book.title, book.author).as_bytes(),
            )
            .await?;
    }
    Ok(())
}

pub async fn choose_rank<S: AsyncRead + AsyncBufReadExt + AsyncWrite + Unpin>(
    stream: &mut S,
    num_items: usize,
) -> anyhow::Result<Option<usize>> {
    enum RankError {
        TooSmall,
        TooLarge,
    }

    if num_items == 0 {
        return Ok(None);
    }

    let min_rank = 1;
    let max_rank = num_items;

    match readln(stream, "which item number? ")
        .await?
        .parse::<usize>()
        .map_err::<(Option<ParseIntError>, Option<RankError>), _>(|err| match err.kind() {
            IntErrorKind::PosOverflow => (Some(err), Some(RankError::TooLarge)),
            _ => (Some(err), None),
        })
        .and_then(|rank| {
            if rank < min_rank {
                Err((None, Some(RankError::TooSmall)))
            } else if max_rank < rank {
                Err((None, Some(RankError::TooLarge)))
            } else {
                Ok(rank)
            }
        }) {
        Ok(rank) => {
            let index = rank.checked_sub(1).unwrap();
            Ok(Some(index))
        }
        Err((_std_err, Some(our_err))) => {
            stream.write_all(b"item number must be ").await?;
            match our_err {
                RankError::TooSmall => {
                    stream
                        .write_all(format!("at least {min_rank}.\n").as_bytes())
                        .await?;
                }
                RankError::TooLarge => {
                    stream
                        .write_all(format!("at most {max_rank}.\n").as_bytes())
                        .await?;
                }
            }
            Ok(None)
        }
        Err((Some(std_err), _our_err)) => match std_err.kind() {
            IntErrorKind::Empty => Ok(None),
            _ => {
                stream.write_all(format!("{std_err}.\n").as_bytes()).await?;
                Ok(None)
            }
        },
        Err((None, None)) => unreachable!(),
    }
}

pub async fn choose_entry<S: AsyncRead + AsyncBufReadExt + AsyncWrite + Unpin>(
    stream: &mut S,
    library: &Library,
    entries: impl ExactSizeIterator<Item = (f64, BookID, Metadata)>,
) -> anyhow::Result<Option<usize>> {
    let len = entries.len();
    enumerate_entries(stream, library, entries).await?;
    choose_rank(stream, len).await
}

pub async fn search<S: AsyncRead + AsyncBufReadExt + AsyncWrite + Unpin>(
    stream: &mut S,
    library: &Library,
) -> anyhow::Result<(String, Vec<(f64, BookID, Metadata)>)> {
    let query = readln(stream, "search query? ").await?;
    let search = library.search(&query).await;

    if search.is_empty() {
        if query.is_empty() {
            stream.write_all(b"the library is empty!\n").await?;
        } else {
            stream.write_all(b"no matching books!\n").await?;
        }
    }

    Ok((query, search))
}

pub async fn do_cmd<S: AsyncRead + AsyncBufReadExt + AsyncWrite + Unpin>(
    stream: &mut S,
    cmd: Command,
    library: &Library,
    guest: IpAddr,
) -> anyhow::Result<Passback> {
    tracing::trace!(cmd = format_args!("{cmd:?}"), "received command");

    match cmd {
        Command::None => {}

        Command::Help => {
            // NOTE: assumes Command names are single-width ASCII ⊆ Unicode characters (which is correct, now).
            let short_long_len = Command::ALL
                .iter()
                .map(|cmd| cmd.short().len() + cmd.long().len())
                .max()
                .expect("Command::ALL must not be empty");
            for cmd in Command::ALL {
                const EXTRA: usize = 8;
                let padding = (short_long_len - (cmd.short().len() + cmd.long().len())) + EXTRA;
                let help_text = match cmd {
                    Command::None => "doesn't do anything.",
                    Command::Help => "ask for assistance.",
                    Command::Quit => "Abandon all Data.",
                    Command::Search => "search the library.",
                    Command::CheckOut => "acquire a book, if it is available!",
                    Command::CheckIn => "return a book.",
                    Command::Read => "peruse your checked out books.",
                    Command::Add => "add a New Book to the library's collection.",
                    Command::Meow => "(warning: meows at you).",
                };

                stream.write_all(cmd.short().as_bytes()).await?;
                stream.write_all(b", ").await?;
                stream.write_all(cmd.long().as_bytes()).await?;
                for _ in 0..padding {
                    stream.write_all(b" ").await?;
                }
                stream.write_all(help_text.as_bytes()).await?;
                stream.write_all(b"\n").await?;
            }
        }

        Command::Search => {
            let (_query, search) = search(stream, library).await?;
            enumerate_entries(stream, library, search.iter().copied()).await?;
        }

        Command::Quit => return Ok(Passback::Quit),

        Command::CheckOut => {
            let (_query, search) = search(stream, library).await?;
            if let Some(index) = choose_entry(stream, library, search.iter().copied()).await? {
                let (_sim, book_id, _meta) = search[index];
                let rank = index + 1;
                match library.checkout(book_id, guest) {
                    Ok(()) => {
                        stream
                            .write_all(format!("checked out item {rank}!\n").as_bytes())
                            .await?;
                    }
                    Err(err) => match err {
                        UpdateEntryError::AlreadyCheckedOut(by) => {
                            stream
                                .write_all(format!("item {rank} is already checked out").as_bytes())
                                .await?;
                            if let Some(by_nick) = library.lookup_guest_by_addr(by).await {
                                stream
                                    .write_all(format!(" by '{by_nick}'").as_bytes())
                                    .await?;
                            }
                            stream.write_all(b".\n").await?;
                        }
                        UpdateEntryError::GuestMismatch | UpdateEntryError::AlreadyCheckedIn => {
                            unreachable!()
                        }
                    },
                }
            } else {
                stream.write_all(b"nevermind.\n").await?;
            }
        }

        Command::CheckIn => {
            let checked_out: Vec<(BookID, Metadata)> =
                library.lookup_checkouts_by_guest(guest).await;
            if checked_out.is_empty() {
                stream.write_all(b"check out some books first!\n").await?;
                return Ok(Passback::Continue);
            }

            if let Some(index) = choose_entry(
                stream,
                library,
                checked_out.iter().map(|&(book, meta)| (1.0, book, meta)),
            )
            .await?
            {
                let (book_id, _meta) = checked_out[index];
                let rank = index + 1;
                match library.checkin(book_id, guest) {
                    Ok(()) => {
                        stream
                            .write_all(format!("returned item {rank}.\n").as_bytes())
                            .await?;
                    }
                    Err(err) => match err {
                        UpdateEntryError::AlreadyCheckedIn => {
                            stream
                                .write_all(
                                    format!("item {rank} is already checked in.\n").as_bytes(),
                                )
                                .await?;
                        }
                        UpdateEntryError::GuestMismatch => {
                            stream
                                .write_all(
                                    format!("item {rank} is checked out by somebody else.\n")
                                        .as_bytes(),
                                )
                                .await?;
                        }
                        UpdateEntryError::AlreadyCheckedOut(_) => unreachable!(),
                    },
                }
            } else {
                stream.write_all(b"nevermind.\n").await?;
            }
        }

        Command::Read => {
            let checked_out: Vec<(BookID, Metadata)> =
                library.lookup_checkouts_by_guest(guest).await;
            if checked_out.is_empty() {
                stream.write_all(b"check out some books first!\n").await?;
                return Ok(Passback::Continue);
            }

            if let Some(index) = choose_entry(
                stream,
                library,
                checked_out.iter().map(|&(book, meta)| (1.0, book, meta)),
            )
            .await?
            {
                let (book_id, meta) = checked_out[index];
                let book: &Book = &*library.lookup_book_by_id(book_id).await;
                editor::read_book(stream, library, book, meta).await?;
            } else {
                stream.write_all(b"nevermind.\n").await?;
            }
        }

        Command::Add => {
            let mut title = String::new();
            let mut author = String::new();
            let mut description = String::new();

            for (dst, prompt) in [
                (&mut title, "Title? "),
                (&mut author, "Author? "),
                (&mut description, "Description? "),
            ] {
                let mut tries = 0;
                const MAX_TRIES: usize = 2;
                while dst.is_empty() {
                    if MAX_TRIES <= tries {
                        stream.write_all(b"nevermind.\n").await?;
                        return Ok(Passback::Continue);
                    }

                    *dst = readln(stream, prompt).await?;
                    tries += 1;
                }
            }

            let mut lines = Vec::new();
            let mut content = String::new();
            {
                let mut editor = Editor::new(&mut lines, false);
                editor.enter(stream).await?;
            }
            for line in lines {
                content.push_str(&line);
                content.push_str("\n");
            }

            stream.write_all(b"adding the book '").await?;
            stream.write_all(title.as_bytes()).await?;
            stream.write_all(b"'...").await?;
            stream.flush().await?;

            let book = Book {
                title,
                author,
                description,
                content,
            };
            library.add(book, guest).await;
            stream.write_all(b"done!\n").await?;
        }

        Command::Meow => {
            stream.write_all(b"meow?\n").await?;
        }
    }

    Ok(Passback::Continue)
}
