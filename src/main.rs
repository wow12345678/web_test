use askama::Template;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse};
use axum::routing::post;
use axum::{Router, routing::get};
use std::fs::read_to_string;
use std::sync::Mutex;
use std::sync::{Arc, PoisonError};
use tower_http::services::ServeFile;

#[tokio::main]
async fn main() {
    let shared_state = Arc::new(Mutex::new(AppState {
        funny_list: vec![
            "hahaah".into(),
            "das".into(),
            "ist".into(),
            "so".into(),
            "lustig".into(),
        ],
        funny_number: 69,
    }));

    let app = Router::new()
        .route("/", get(homepage))
        .route("/about", get(about))
        .route("/chat_test", get(chat_test))
        .route("/search", get(search))
        .route("/funny_list", get(lister))
        .route_service("/cargo", ServeFile::new("Cargo.toml"))
        .route("/add", post(adder))
        .with_state(shared_state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    println!(
        "Server running at http://{}",
        listener.local_addr().unwrap()
    );
    axum::serve(listener, app).await.unwrap();
}

#[derive(Debug)]
struct AppState {
    funny_list: Vec<String>,
    funny_number: u32,
}

#[derive(Template, Debug)]
#[template(path = "lister.html")]
struct FunnyList {
    funny_items: Vec<String>,
}

async fn adder(State(state): State<Arc<Mutex<AppState>>>) -> impl IntoResponse {
    println!("button pressed 😄");
    let mut state_lock = state.lock().unwrap_or_else(|err| {
        println!("The mutex was poisoned 😱");
        PoisonError::into_inner(err)
    });
    state_lock.funny_number += 1;
    state_lock.funny_list.push("woow".into());

    Html("Click Me!")
}

async fn homepage() -> Html<String> {
    let content = read_to_string("templates/index.html").unwrap();
    Html(content)
}

async fn lister(State(state): State<Arc<Mutex<AppState>>>) -> Result<impl IntoResponse, AppError> {
    let state_lock = state.lock().unwrap_or_else(|err| {
        println!("The mutex was poisoned 😱");
        PoisonError::into_inner(err)
    });
    let funny_list = FunnyList {
        funny_items: state_lock.funny_list.clone(),
    };
    Ok(Html(funny_list.render()?))
}

async fn about() -> Html<String> {
    let content = read_to_string("templates/about.html").unwrap();
    Html(content)
}

async fn chat_test() -> Html<String> {
    let content = read_to_string("templates/chat_test.html").unwrap();
    Html(content)
}

async fn search() -> Html<String> {
    let content = read_to_string("templates/search.html").unwrap();
    Html(content)
}

#[derive(Debug, displaydoc::Display, thiserror::Error)]
enum AppError {
    /// could not render template
    Render(#[from] askama::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        #[derive(Debug, Template)]
        #[template(
            ext = "txt",
            source = r#"
    error has occurred with status code {{ status_code }} and message {{ message }}
        "#
        )]
        struct Tmpl {
            message: String,
            status_code: u16,
        }

        let (status, message) = match &self {
            AppError::Render(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            ),
        };
        let tmpl = Tmpl {
            status_code: status.as_u16(),
            message,
        };

        if let Ok(body) = tmpl.render() {
            (status, Html(body)).into_response()
        } else {
            (status, "Something went wrong").into_response()
        }
    }
}
