mod library {
    use crate::library::{Book, Library, Metadata, UpdateEntryError};

    #[tokio::test]
    async fn add_and_search() {
        let book = Book {
            title: String::from("foo"),
            author: String::from("cat 1"),
            description: String::from("bar"),
            content: String::from("baz"),
        };
        let lib = Library::new();
        let guest = Library::OPERATOR;
        let id = lib.add(book, guest).await;
        assert_eq!(vec![(1.0, id, Metadata::new(guest))], lib.search("").await);
        assert_eq!(
            vec![(1.0, id, Metadata::new(guest))],
            lib.search("foo").await
        );
    }

    #[tokio::test]
    async fn add_many() {
        let lib = Library::new();
        let book = Book {
            title: String::from("foo"),
            author: String::from("cat 1"),
            description: String::from("bar"),
            content: String::from("baz"),
        };
        let book2 = Book {
            title: String::from("foo"),
            author: String::from("cat 1"),
            description: String::from("bar"),
            content: String::from("haha!"),
        };
        let guest = Library::OPERATOR;
        {
            let mut expect = Vec::new();
            for _ in 1..=3 {
                let id = lib.add(book.clone(), guest).await;
                let meta = lib.lookup_metadata(id);
                expect.push((1.0, id, meta));
                assert_eq!(expect, lib.search("").await);
            }
        }
        let id2 = lib.add(book2.clone(), guest).await;
        assert_eq!(
            vec![(1.0, id2, Metadata::new(guest))],
            lib.search("haha!").await
        );
    }

    #[tokio::test]
    async fn checkout_and_checkin() {
        let book = Book {
            title: String::from("foo"),
            author: String::from("cat 1"),
            description: String::from("bar"),
            content: String::from("baz"),
        };
        let lib = Library::new();
        let guest = Library::OPERATOR;
        let id = lib.add(book.clone(), guest).await;

        assert_eq!(Ok(()), lib.checkout(id, guest));
        assert_eq!(
            Err(UpdateEntryError::AlreadyCheckedOut(guest)),
            lib.checkout(id, guest)
        );
        assert_eq!(Ok(()), lib.checkin(id, guest));
        assert_eq!(
            Err(UpdateEntryError::AlreadyCheckedIn),
            lib.checkin(id, guest)
        );
        assert_eq!(Ok(()), lib.checkout(id, guest));
    }
}
