use core::cmp;
use std::borrow::Cow;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt};

use crate::library::{Book, Library, Metadata};
use crate::shell;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Passback {
    Quit,
    Continue,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Quit,
    Help,
    Print,
    CountLines,
    LineNext(usize),
    LinePrev(usize),
    LineGotoIdx(usize),
    // SetSearch(String),
    // SearchPrev,
    // SearchNext,

    // !readonly
    Insert,
    Append,
    Change,
    Delete,
}

impl Command {
    pub async fn build<S: AsyncRead + AsyncBufReadExt + AsyncWrite + Unpin>(
        stream: &mut S,
        num_lines: usize,
    ) -> anyhow::Result<Option<Self>> {
        let try_cmd = shell::readln(stream, ":").await?;

        for (prefix, offset, ctor) in [
            ("", 1, Self::LineGotoIdx as fn(usize) -> Self),
            ("j", 0, Self::LineNext),
            ("k", 0, Self::LinePrev),
        ] {
            if let Some(try_by) = try_cmd.strip_prefix(prefix) {
                if let Ok(num) = try_by.parse::<usize>() {
                    let adjusted = num.saturating_sub(offset);
                    return Ok(Some(ctor(adjusted)));
                }
            }
        }

        let cmd = match try_cmd.as_str() {
            "q" | "quit" => Self::Quit,
            "?" | "h" | "help" => Self::Help,
            "p" => Self::Print,
            "l" => Self::CountLines,
            "" | "j" => Self::LineNext(1),
            "k" => Self::LinePrev(1),
            "g" => Self::LineGotoIdx(0),
            "G" => Self::LineGotoIdx(num_lines.saturating_sub(1)),
            "i" => Self::Insert,
            "a" => Self::Append,
            "c" => Self::Change,
            "d" => Self::Delete,
            _ => return Ok(None),
        };
        Ok(Some(cmd))
    }
}

pub struct Editor<'vec, 'src> {
    lines: &'vec mut Vec<Cow<'src, str>>,
    readonly: bool,

    // NOTE: always refers to a valid line
    cur_line: usize,
    prev_line_printed: Option<usize>,
    linum_pad: usize,

    prev_cmd: Option<Command>,
}

impl<'vec, 'src> Editor<'vec, 'src> {
    pub fn new(lines: &'vec mut Vec<Cow<'src, str>>, readonly: bool) -> Self {
        let mut editor = Self {
            lines,
            readonly,

            cur_line: 0,
            prev_line_printed: None,
            linum_pad: 0,

            prev_cmd: None,
        };

        editor.recompute_pad();
        editor
    }

    pub fn num_lines(&self) -> usize {
        self.lines.len()
    }

    fn recompute_pad(&mut self) {
        let pad = usize::checked_ilog10(self.lines.len()).unwrap_or(0) + 1;
        let pad = usize::try_from(pad).unwrap_or(usize::MAX);
        self.linum_pad = pad;
    }

    fn fmt_margin(pad: usize, idx: usize) -> String {
        let linum = idx + 1;
        format!("{linum:>pad$} |	")
    }

    fn fmt_line(pad: usize, lines: &[Cow<'_, str>], idx: usize) -> String {
        let line = &lines[idx];
        let margin = Self::fmt_margin(pad, idx);
        format!("{margin}{line}\n")
    }

    fn clamp_line(&mut self) {
        self.cur_line = cmp::min(self.cur_line, self.lines.len().saturating_sub(1));
        if self.lines.is_empty() {
            self.lines.push(Cow::Borrowed(""));
        }
    }

    async fn insert_lines_at<S: AsyncRead + AsyncBufReadExt + AsyncWrite + Unpin>(
        &mut self,
        stream: &mut S,
        start_idx: usize,
    ) -> anyhow::Result<()> {
        self.cur_line = start_idx;
        loop {
            let prompt = Self::fmt_margin(self.linum_pad, self.cur_line);
            let line = shell::readln(stream, &prompt).await?;

            if line == "." {
                self.prev_line_printed = Some(self.cur_line);
                break;
            }

            self.recompute_pad();
            self.lines.insert(self.cur_line, Cow::Owned(line));
            self.cur_line += 1;
        }

        Ok(())
    }

    fn print_range(&self) -> impl Iterator<Item = usize> {
        let skip_prev = self.prev_line_printed.is_some();
        let skip_cur = Some(self.cur_line) == self.prev_line_printed;
        (self.prev_line_printed.unwrap_or(0)..self.cur_line)
            .skip(if skip_prev { 1 } else { 0 })
            .chain(if skip_cur { None } else { Some(self.cur_line) })
    }

    async fn print<S: AsyncRead + AsyncBufReadExt + AsyncWrite + Unpin>(
        &mut self,
        stream: &mut S,
    ) -> anyhow::Result<()> {
        /* make sure current line is valid index */
        self.clamp_line();

        /* determine whether to move the cursor back. this only makes sense if
         * we are trying to hide the prior prompt, to prevent broken up buffer
         * lines. so, we need to make sure that the prior line is really the
         * prompt. */
        let directly_printed_line_prev = match self.prev_cmd {
            Some(Command::LineGotoIdx(idx)) if Some(idx) < self.prev_line_printed => true,
            Some(Command::LinePrev(_))
            | Some(Command::Print)
            | Some(Command::Insert)
            | Some(Command::Append)
            | Some(Command::Change)
            | Some(Command::Delete)
            | None => true,
            _ => false,
        };
        let will_print_buf_lines = self.print_range().next().is_some();
        if !directly_printed_line_prev && will_print_buf_lines {
            shell::move_cursor_prev(stream).await?;
        }

        /* print whatever range of lines needs to be visually updated */
        for idx in self.print_range() {
            let line = Self::fmt_line(self.linum_pad, &self.lines, idx);
            shell::clear_line(stream).await?;
            stream.write_all(line.as_bytes()).await?;
            self.prev_line_printed = Some(idx);
        }

        Ok(())
    }

    async fn handle_cmd<S: AsyncRead + AsyncBufReadExt + AsyncWrite + Unpin>(
        &mut self,
        stream: &mut S,
        cmd: Command,
    ) -> anyhow::Result<Passback> {
        match (self.readonly, cmd) {
            (_, Command::Quit) => return Ok(Passback::Quit),

            (_, Command::Help) => {
                const HELP: &'static [(bool, &str, &str)] = &[
                    (false, "q, quit", "quit reading."),
                    (false, "?, h, help", "list commands."),
                    (false, "print", "print first through current lines."),
                    (false, "l, lines", "print line count."),
                    (
                        false,
                        "<enter>, j, j<N>",
                        "move by next N lines [default: 1].",
                    ),
                    (false, "k, k<N>", "move by previous N lines [default: 1]."),
                    (false, "g", "goto first line."),
                    (false, "G", "goto last line."),
                    (false, "<N>", "goto line N."),
                    (true, "i", "insert new line before."),
                    (true, "a", "insert new line after."),
                    (true, "c", "replace current line."),
                    (true, "d", "delete current line."),
                ];
                let max_left = HELP.iter().map(|t| t.1.chars().count()).max().unwrap();
                let help_pad = max_left + 8;
                for &(writes, left, right) in HELP {
                    if self.readonly && writes {
                        continue;
                    }
                    stream
                        .write_all(format!(" {left:<help_pad$}{right}\n").as_bytes())
                        .await?;
                }
            }

            (_, Command::Print) => {
                self.prev_line_printed = None;
            }

            (_, Command::CountLines) => {
                stream
                    .write_all(format!("{}\n", self.num_lines()).as_bytes())
                    .await?;
            }

            (_, Command::LineNext(by)) => {
                self.cur_line = self.cur_line.saturating_add(by);
            }

            (_, Command::LinePrev(by)) => {
                self.cur_line = self.cur_line.saturating_sub(by);
            }

            (_, Command::LineGotoIdx(index)) => {
                self.prev_line_printed = Some(self.cur_line);
                self.cur_line = index;
            }

            (true, _) => {
                stream.write_all(b"can't edit readonly buffer.\n").await?;
            }

            (false, Command::Insert) => {
                self.insert_lines_at(stream, self.cur_line).await?;
            }

            (false, Command::Append) => {
                self.insert_lines_at(stream, self.cur_line + 1).await?;
            }

            (false, Command::Change) => {
                let idx = self.cur_line;
                let prompt = Self::fmt_margin(self.linum_pad, idx);
                let line = shell::readln(stream, &prompt).await?;
                self.lines[idx] = Cow::Owned(line);
                self.prev_line_printed = Some(idx);
            }

            (false, Command::Delete) => {
                let idx = self.cur_line;
                self.lines.remove(idx);
                self.prev_line_printed = None;
            }
        }
        self.prev_cmd = Some(cmd);

        Ok(Passback::Continue)
    }

    pub async fn enter<S: AsyncRead + AsyncBufReadExt + AsyncWrite + Unpin>(
        &mut self,
        stream: &mut S,
    ) -> anyhow::Result<()> {
        'outer: loop {
            /* print buffer */
            self.print(stream).await?;

            /* take command */
            if let Some(cmd) = Command::build(stream, self.num_lines()).await? {
                match self.handle_cmd(stream, cmd).await? {
                    Passback::Continue => continue 'outer,
                    Passback::Quit => break 'outer,
                }
            } else {
                stream
                    .write_all(b"unknown command. type \"help\".\n")
                    .await?;
            }
        }
        Ok(())
    }
}

pub async fn cover_page<S: AsyncRead + AsyncBufReadExt + AsyncWrite + Unpin>(
    stream: &mut S,
    library: &Library,
    book: &Book,
    meta: Metadata,
) -> anyhow::Result<()> {
    stream.write_all(b"\n").await?;
    stream
        .write_all(format!("	'{}'\n", book.title).as_bytes())
        .await?;
    stream
        .write_all(format!("		by {}\n", book.author).as_bytes())
        .await?;
    if !book.description.is_empty() {
        stream.write_all(b"\n").await?;
    }
    for line in book.description.lines() {
        stream.write_all(format!("	{line}\n").as_bytes()).await?;
    }
    stream.write_all(b"\n").await?;
    stream
        .write_all(
            format!(
                "	[Total {} checkout{}.]\n",
                meta.checkouts,
                if meta.checkouts == 1 { "" } else { "s" }
            )
            .as_bytes(),
        )
        .await?;
    if let Some(nick) = library.lookup_guest_by_addr(meta.added_by).await {
        stream
            .write_all(format!("	[Added by guest '{nick}'.]\n").as_bytes())
            .await?;
    }
    stream.write_all(b"\n").await?;

    Ok(())
}

pub async fn read_book<S: AsyncRead + AsyncBufReadExt + AsyncWrite + Unpin>(
    stream: &mut S,
    library: &Library,
    book: &Book,
    meta: Metadata,
) -> anyhow::Result<()> {
    /* cover page */
    cover_page(stream, library, book, meta).await?;

    /* readonly edit view over book contents */
    let mut lines: Vec<Cow<'_, str>> = book.content.lines().map(Cow::Borrowed).collect();
    let readonly = true;
    let mut editor = Editor::new(&mut lines, readonly);
    editor.enter(stream).await?;

    Ok(())
}
