use core::cmp::Ordering;
use core::net::{IpAddr, Ipv4Addr};
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Book {
    pub title: String,
    pub author: String,
    pub description: String,
    pub content: String,
}

// TODO: (title, author) should be sacred

impl PartialOrd for Book {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        Some(self.title.cmp(&rhs.title))
    }
}

impl Ord for Book {
    fn cmp(&self, rhs: &Self) -> Ordering {
        self.title.cmp(&rhs.title)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Metadata {
    pub added_by: IpAddr,
    pub checkouts: u64,
    pub checked_out_by: Option<IpAddr>,
}

impl Metadata {
    pub fn new(added_by: IpAddr) -> Self {
        Self {
            added_by,
            checkouts: 0,
            checked_out_by: None,
        }
    }

    pub const fn is_free(&self) -> bool {
        self.checked_out_by.is_none()
    }

    pub fn register_checkout(&mut self) {
        self.checkouts = self.checkouts.saturating_add(1);
    }

    pub fn set_checkout(&mut self, guest: IpAddr) -> Option<IpAddr> {
        let old = self.checked_out_by;
        self.checked_out_by = Some(guest);
        old
    }

    pub fn set_checkin(&mut self) -> Option<IpAddr> {
        self.checked_out_by.take()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateEntryError {
    AlreadyCheckedOut(IpAddr),
    AlreadyCheckedIn,
    GuestMismatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegisterError {
    AlreadyRegistered,
    NicknameTaken,
}

#[derive(Clone, Debug)]
pub struct Guest {
    addr: IpAddr,
    pub nick: Arc<str>,
}

impl Guest {
    pub fn new(addr: IpAddr, nick: &str) -> Self {
        Self {
            addr,
            nick: Arc::from(nick),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct BookID(usize);

#[derive(Debug)]
pub struct Library {
    /// Push-only pool of books. Indices are unique and stable mappings to books.
    book_pool: RwLock<Vec<Arc<Book>>>,

    /// Table of book metadata. This is expected to be frequently read and
    /// written to as books are checked in and out.
    book_meta: DashMap<BookID, Metadata>,

    // NOTE: (sorted ascending by IpAddr, sorted ascending by nickname)
    guests: RwLock<(Vec<Guest>, Vec<Arc<str>>)>,
}

impl Library {
    pub const OPERATOR: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);

    pub fn new() -> Self {
        let operator = Guest::new(Self::OPERATOR, "cat in the machine");
        Self {
            book_pool: RwLock::new(Vec::new()),
            book_meta: DashMap::new(),
            guests: RwLock::new((vec![operator.clone()], vec![operator.nick])),
        }
    }

    pub async fn with_collection<I: IntoIterator<Item = Book>>(collection: I) -> Self {
        let lib = Self::new();
        for book in collection {
            lib.add(book, Self::OPERATOR).await;
        }
        lib
    }

    pub async fn lookup_guest_by_addr(&self, addr: IpAddr) -> Option<Arc<str>> {
        let (guests, _nicks) = &*self.guests.read().await;
        match guests.binary_search_by_key(&addr, |guest| guest.addr) {
            Ok(idx) => Some(Arc::clone(&guests[idx].nick)),
            Err(_idx) => None,
        }
    }

    pub async fn register_guest(
        &self,
        addr: IpAddr,
        nick: impl Into<Arc<str>>,
    ) -> Result<Arc<str>, RegisterError> {
        let (ref mut guests, nicks) = &mut *self.guests.write().await;

        let nick: Arc<str> = nick.into();

        /* check if this nickname is taken */
        match nicks.binary_search(&nick) {
            Ok(_idx) => return Err(RegisterError::NicknameTaken),
            Err(idx) => nicks.insert(idx, Arc::clone(&nick)),
        }

        /* associate address with nickname */
        match guests.binary_search_by_key(&addr, |guest| guest.addr) {
            Ok(_) => Err(RegisterError::AlreadyRegistered),
            Err(idx) => {
                let guest = Guest {
                    addr,
                    nick: Arc::clone(&nick),
                };
                guests.insert(idx, guest);
                Ok(nick)
            }
        }
    }

    pub async fn is_empty(&self) -> bool {
        self.book_pool.read().await.is_empty()
    }

    pub async fn lookup_book_by_id(&self, id: BookID) -> Arc<Book> {
        let pool = self.book_pool.read().await;
        let book = Arc::clone(&pool[id.0]);
        book
    }

    pub async fn lookup_checkouts_by_guest(&self, guest: IpAddr) -> Vec<(BookID, Metadata)> {
        // TODO: inefficient

        let mut found = Vec::new();
        for entry in self.book_meta.iter() {
            let (&id, &meta) = entry.pair();
            if meta.checked_out_by == Some(guest) {
                found.push((id, meta));
            }
        }
        found
    }

    pub fn lookup_metadata(&self, id: BookID) -> Metadata {
        *self.book_meta.get(&id).unwrap()
    }

    pub async fn search(&self, query: &str) -> Vec<(f64, BookID, Metadata)> {
        fn cmp(book: &Book, query: &str) -> Option<f64> {
            if query == "" {
                return Some(1.0);
            }

            const THRESHOLD: f64 = 0.4;
            let mut sim = None;

            let query_len = query.chars().count();

            for src in [&book.title, &book.author, &book.description, &book.content] {
                /* compare whole similarity */
                let whole_sim = strsim::normalized_damerau_levenshtein(query, src);

                /* compare percentage of containment */
                let instances = src.match_indices(query).count();
                let instances_len = instances * query_len;
                let substr_sim = if instances_len == 0 {
                    0.0
                } else {
                    src.len() as f64 / instances_len as f64
                };

                for cur in [whole_sim, substr_sim] {
                    match sim {
                        None => sim = Some(cur),
                        Some(prev) => {
                            if prev < cur {
                                sim = Some(cur);
                            }
                        }
                    }
                }
            }

            let sim = sim.unwrap();

            if THRESHOLD <= sim {
                Some(sim)
            } else {
                None
            }
        }

        let mut found = Vec::new();
        {
            let pool = self.book_pool.read().await;
            for (idx, book) in pool.iter().enumerate() {
                let book_id = BookID(idx);
                if let Some(sim) = cmp(book, query) {
                    let meta = self.lookup_metadata(book_id);
                    found.push((sim, book_id, meta));
                }
            }
        }
        // HA HA HA
        found.sort_by(|(a, _, _), (b, _, _)| b.partial_cmp(a).unwrap_or(Ordering::Less));

        found
    }

    pub async fn add(&self, book: impl Into<Arc<Book>>, guest: IpAddr) -> BookID {
        let mut pool = self.book_pool.write().await;
        let book_id: BookID = BookID(pool.len());
        let book: Arc<Book> = book.into();
        pool.push(book);

        let old = self.book_meta.insert(book_id, Metadata::new(guest));
        debug_assert!(
            old.is_none(),
            "it would be weird if this BookID already existed"
        );

        book_id
    }

    pub fn checkout(&self, book_id: BookID, guest: IpAddr) -> Result<(), UpdateEntryError> {
        let mut meta = self.book_meta.get_mut(&book_id).unwrap();
        match meta.checked_out_by {
            Some(by) => Err(UpdateEntryError::AlreadyCheckedOut(by)),
            None => {
                meta.set_checkout(guest);
                meta.register_checkout();
                Ok(())
            }
        }
    }

    pub fn checkin(&self, book_id: BookID, guest: IpAddr) -> Result<(), UpdateEntryError> {
        let mut meta = self.book_meta.get_mut(&book_id).unwrap();
        if let Some(by) = meta.checked_out_by {
            if by == guest {
                meta.set_checkin();
                Ok(())
            } else {
                Err(UpdateEntryError::GuestMismatch)
            }
        } else {
            Err(UpdateEntryError::AlreadyCheckedIn)
        }
    }
}
