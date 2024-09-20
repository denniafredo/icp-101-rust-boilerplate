#[macro_use]
extern crate serde;
use candid::{Decode, Encode};
use ic_cdk::api::time;
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{BoundedStorable, Cell, DefaultMemoryImpl, StableBTreeMap, Storable};
use std::{borrow::Cow, cell::RefCell};

type Memory = VirtualMemory<DefaultMemoryImpl>;
type IdCell = Cell<u64, Memory>;

#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct Book {
    id: u64,
    title: String,
    author: String,
    summary: String,
    year: u64,
    created_at: u64,
    updated_at: Option<u64>,
}

// Implement the Storable trait for Book
impl Storable for Book {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

// Implement BoundedStorable for Book
impl BoundedStorable for Book {
    const MAX_SIZE: u32 = 1024;
    const IS_FIXED_SIZE: bool = false;
}

thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(
        MemoryManager::init(DefaultMemoryImpl::default())
    );

    static ID_COUNTER: RefCell<IdCell> = RefCell::new(
        IdCell::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))), 0)
            .expect("Cannot create a counter")
    );

    static STORAGE: RefCell<StableBTreeMap<u64, Book, Memory>> =
        RefCell::new(StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1)))
    ));
}

#[derive(candid::CandidType, Serialize, Deserialize, Default)]
struct BookPayload {
    title: String,
    author: String,
    summary: String,
    year: u64,
}

#[ic_cdk::query]
fn get_book(id: u64) -> Result<Book, Error> {
    match _get_book(&id) {
        Some(book) => Ok(book),
        None => Err(Error::NotFound {
            msg: format!("A book with id={} not found", id),
        }),
    }
}

#[ic_cdk::update]
fn add_book(payload: BookPayload) -> Option<Book> {
    let id = ID_COUNTER
        .with(|counter| {
            let current_value = *counter.borrow().get();
            counter.borrow_mut().set(current_value + 1)
        })
        .expect("Cannot increment id counter");
    let book = Book {
        id,
        title: payload.title,
        author: payload.author,
        summary: payload.summary,
        year: payload.year,
        created_at: time(),
        updated_at: None,
    };
    do_insert(&book);
    Some(book)
}

#[ic_cdk::update]
fn update_book(id: u64, payload: BookPayload) -> Result<Book, Error> {
    match STORAGE.with(|service| service.borrow().get(&id)) {
        Some(mut book) => {
            book.title = payload.title;
            book.author = payload.author;
            book.summary = payload.summary;
            book.year = payload.year;
            book.updated_at = Some(time());
            do_insert(&book);
            Ok(book)
        }
        None => Err(Error::NotFound {
            msg: format!("Couldn't update a book with id={}. Book not found", id),
        }),
    }
}

fn do_insert(book: &Book) {
    STORAGE.with(|service| service.borrow_mut().insert(book.id, book.clone()));
}

#[ic_cdk::update]
fn delete_book(id: u64) -> Result<Book, Error> {
    match STORAGE.with(|service| service.borrow_mut().remove(&id)) {
        Some(book) => Ok(book),
        None => Err(Error::NotFound {
            msg: format!("Couldn't delete a book with id={}. Book not found.", id),
        }),
    }
}

#[derive(candid::CandidType, Deserialize, Serialize)]
enum Error {
    NotFound { msg: String },
}

fn _get_book(id: &u64) -> Option<Book> {
    STORAGE.with(|service| service.borrow().get(id))
}

// Export candid interface for external usage
ic_cdk::export_candid!();
