use askama::Template;
use axum::extract::{Form, State};
use axum::http::{Response, StatusCode};
use axum::response::{Html, IntoResponse, Redirect};
use axum::routing::post;
use axum::{Router, routing::get};
use bcrypt::{DEFAULT_COST, verify};
use std::collections::HashMap;
use std::fs::read_to_string;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, Mutex, PoisonError};
use toml::{Table, Value, map::Map};
use tower_cookies::{Cookie, CookieManagerLayer, Cookies, Key};

struct AppState {
    users: Map<String, Value>,
    current_users: HashMap<String, u64>,
    cookie_key: Key,
}

#[tokio::main]
async fn main() {
    let passwords = read_to_string("users.toml").unwrap();
    let users = passwords.parse::<Table>().unwrap();

    let cookie_key = Key::generate();

    let state = Arc::new(Mutex::new(AppState {
        users,
        current_users: HashMap::new(),
        cookie_key,
    }));

    let app = Router::new()
        .route("/", get(homepage))
        .route(
            "/chat/ana",
            get(|cookies: Cookies, state: State<Arc<Mutex<AppState>>>| {
                chat(ChatPageType::Ana, cookies, state)
            }),
        )
        .route(
            "/chat/la",
            get(|cookies: Cookies, state: State<Arc<Mutex<AppState>>>| {
                chat(ChatPageType::La, cookies, state)
            }),
        )
        .route(
            "/chat/eaz",
            get(|cookies: Cookies, state: State<Arc<Mutex<AppState>>>| {
                chat(ChatPageType::Eaz, cookies, state)
            }),
        )
        .route("/login", get(login_page))
        .route("/login", post(login_handler))
        .layer(CookieManagerLayer::new())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    println!(
        "Server running at http://{}",
        listener.local_addr().unwrap()
    );
    axum::serve(listener, app).await.unwrap();
}

async fn login_page() -> Html<String> {
    let content = read_to_string("templates/login.html")
        .unwrap_or_else(|_| "<h1>Login Page Not Found</h1>".to_string());
    Html(content)
}

async fn login_handler(
    State(state): State<Arc<Mutex<AppState>>>,
    cookies: Cookies,
    Form(form): Form<LoginForm>,
) -> Html<String> {
    let username = form.username.clone();
    let password = form.password.clone();
    let state_lock = state.lock().unwrap_or_else(|poison_error| {
        println!("Mutex was poisoned!");
        PoisonError::into_inner(poison_error)
    });

    if state_lock
        .users
        .get(&username)
        .is_some_and(|p| verify(password, p["password"].as_str().unwrap()).unwrap())
    {
        let private_jar = cookies.private(&state_lock.cookie_key);
        private_jar.add(
            Cookie::build(("auth", username))
                .secure(true)
                .http_only(true)
                .build(),
        );

        Html("<div class='success' style='color: green; padding: 10px; margin-top: 10px;'>Login successful!</div>".to_string())
    } else {
        Html("<div class='error' style='color: red; padding: 10px; margin-top: 10px;'>Invalid username or password</div>".to_string())
    }
}

#[derive(Debug, serde::Deserialize)]
struct LoginForm {
    username: String,
    password: String,
}

#[derive(Template)]
#[template(path = "chat.html")]
struct ChatPage {
    is_logged_in: bool,
    c_type: ChatPageType,
}
enum ChatPageType {
    Ana,
    La,
    Eaz,
}

async fn homepage() -> Html<String> {
    let content = read_to_string("templates/index.html").unwrap();
    Html(content)
}

async fn chat(
    c_type: ChatPageType,
    cookies: Cookies,
    State(state): State<Arc<Mutex<AppState>>>,
) -> Result<impl IntoResponse, AppError> {
    let state_lock = state.lock().unwrap();
    let private_jar = cookies.private(&state_lock.cookie_key);

    let is_logged_in = private_jar.get("auth").is_some();

    let chat_page = ChatPage {
        is_logged_in,
        c_type,
    };
    Ok(Html(chat_page.render()?))
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
