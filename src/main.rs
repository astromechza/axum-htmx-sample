// Apply the stricter clippy rules to the whole module.
#![deny(clippy::unwrap_used,clippy::expect_used,clippy::panic)]

use anyhow::anyhow;
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::{Form, Router};
use axum::routing::{get, post};
use maud::{html, Markup, DOCTYPE};
use serde::Deserialize;
use crate::htmx::HtmxContext;

/// Our main method runs on the main tokio multi thread runtime and runs the fallible variant,
/// printing any error to stderr.
#[tokio::main]
async fn main() {
    if let Err(e) = main_err().await {
        eprintln!("{}", e);
    }
}

/// The real main function.
async fn main_err() -> Result<(), anyhow::Error> {
    let app = Router::new()
        .route("/", get(home_handler))
        .route("/fallible", get(fallible_handler))
        // we use the same route here for the get and post. This simplifies the progressive enhancement
        // on the form display.
        .route("/form-example", get(form_example))
        .route("/form-example", post(form_example_submit))
        // the fallback applies for 405 and 404
        .fallback(not_found_handler);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:9000").await?;
    axum::serve(listener, app).await?;
    Ok(())
}

/// Our handlers return a [ResponseError] which implements [IntoResponse]. To make the errors more
/// efficient to render, we also capture the [HtmxContext] so that we can determine whether to
/// render the entire html or just swap in the error report content.
#[derive(Debug)]
struct ResponseError {
    /// This is needed so that we know whether to render the whole html or just return the boosted
    /// body content.
    htmx_context: Option<HtmxContext>,
    /// The inner error.
    err: anyhow::Error,
}

impl IntoResponse for ResponseError {
    fn into_response(self) -> Response {
        render_body_html_or_htmx(StatusCode::INTERNAL_SERVER_ERROR, "Internal Error", html! {
            header {
                (render_nav_links())
                h1 { "Internal error" }
            }
            section {
                p {
                    "An internal error has occurred. Please navigate back using the links above."
                }
                code {
                    (self.err)
                }
            }
        }, self.htmx_context)
    }
}

/// This trait helps to attach the [HtmxContext] to the [Result] and convert any old error into
/// a [ResponseError]. We implement this internal trait for any [Result] type.
trait CanMapToRespErr<T> {
    fn map_resp_err(self, htmx: Option<HtmxContext>) -> Result<T, ResponseError>;
}

impl<T, E> CanMapToRespErr<T> for Result<T, E> where E: Into<anyhow::Error> {
    fn map_resp_err(self, htmx: Option<HtmxContext>) -> Result<T, ResponseError> {
        self.map_err(|e| ResponseError{htmx_context: htmx, err: e.into()})
    }
}

/// Renders the main html layout with given title and inner markup.
fn render_body_html(title: impl AsRef<str>, inner: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html {
            head {
                title { (title.as_ref()) }
                script src="https://cdnjs.cloudflare.com/ajax/libs/htmx/2.0.4/htmx.min.js" integrity="sha512-2kIcAizYXhIn8TzUvqzEDZNuDZ+aW7yE/+f1HJHXFjQcGNfv1kqzJSTBRBSlOgp6B/KZsz1K0a3ZTqP9dnxioQ==" crossorigin="anonymous" referrerpolicy="no-referrer" {};
            }
            body hx-boost="true" id="body" {
                (inner)
            }
        }
    }
}

/// Renders either the whole main html, or returns just the content suitable for swapping into the main element.
fn render_body_html_or_htmx(code: StatusCode, title: impl AsRef<str>, inner: Markup, htmx_context: Option<HtmxContext>) -> Response {
    let mut hm = HeaderMap::new();
    hm.insert("Content-Type", HeaderValue::from_static("text/html"));
    if let Some(hc) = htmx_context {
        // Ensure that we retarget the request if it's attempting to swap to the wrong place.
        if hc.target.is_some_and(|x| x.ne("#body")) {
            hm.insert("HX-Retarget", HeaderValue::from_static("#body"));
            hm.insert("HX-Reswap", HeaderValue::from_static("innerHTML"));
        }
        // HTMX requires HTTP 200 responses by default.
        (StatusCode::OK, hm, html! {
            title { (title.as_ref()) }
            (inner)
        }.0).into_response()
    } else {
        (code, hm, render_body_html(title, inner).0).into_response()
    }
}

/// The nav links at the top of the page are always repeated.
fn render_nav_links() -> Markup {
    html! {
        nav {
            a href="/" { "home "}
            " | "
            a href="/fallible" { "fallible" }
            " | "
            a href="/does-not-exist" { "does-not-exist" }
            " | "
            a href="/form-example" { "form-example" }
        }
    }
}

async fn home_handler(headers: HeaderMap) -> Result<Response, ResponseError> {
    Ok(render_body_html_or_htmx(StatusCode::OK, "Home page", html! {
        header {
            (render_nav_links())
            h1 { "Home" }
        }
        section {
            p {
                "This is the home page."
            }
        }
    }, HtmxContext::try_from(headers).ok()).into_response())
}

async fn fallible_handler(headers: HeaderMap) -> Result<Response, ResponseError> {
    let htmx_context = HtmxContext::try_from(headers).ok();

    // Produce an error response sometimes.
    if rand::random::<bool>() {
        Err(anyhow!("request was unlucky")).map_resp_err(htmx_context.clone())?
    }

    Ok(render_body_html_or_htmx(StatusCode::OK, "Lucky!", html! {
        header {
            (render_nav_links())
            h1 { "Lucky you" }
        }
        section {
            p {
                "You were lucky!"
            }
        }
    }, htmx_context).into_response())
}

async fn form_example(headers: HeaderMap) -> Result<Response, ResponseError> {
    let htmx_context = HtmxContext::try_from(headers).ok();
    Ok(render_body_html_or_htmx(StatusCode::OK, "Example form", form_example_body(
        None, None, FormExamplePayload::default(),
    ), htmx_context).into_response())
}

/// The page body of the form page. We use this in a few places.
fn form_example_body(success_message: Option<String>, previous_error: Option<String>, previous_payload: FormExamplePayload) -> Markup {
    html! {
        header {
            (render_nav_links())
            h1 { "Example form" }
        }
        section {
            @if let Some(success_message) = success_message {
                div {
                    strong { "Success" }
                    p { (success_message) }
                }
            }
            @if let Some(previous_error) = previous_error {
                div {
                    strong { "Error" }
                    p { (previous_error) }
                }
            }
            form action="/form-example" method="post" {
                input type="text" name="content" value=(previous_payload.content);
                button type="submit" { "Submit" }
            }
        }
    }
}

#[derive(Debug,Default,Deserialize)]
struct FormExamplePayload {
    content: String,
}

async fn form_example_submit(headers: HeaderMap, Form(payload): Form<FormExamplePayload>) -> Result<Response, ResponseError> {
    let htmx_context = HtmxContext::try_from(headers).ok();

    // validation of the payload
    if let Result::<(), anyhow::Error>::Err(e) = if payload.content.is_empty() {
        Err(anyhow!("Content is empty"))
    } else if !payload.content.is_ascii() {
        Err(anyhow!("Content is not ascii"))
    } else {
        Ok(())
    } {
        // NOTE: we could optimise this by just returning the success or validation messages. But that's only
        // useful if we have expensive content on the page that we don't want to rebuild or render.
        return Ok(render_body_html_or_htmx(StatusCode::BAD_REQUEST, "Example form", form_example_body(
            None, Some(e.to_string()), payload,
        ), htmx_context).into_response());
    }

    Ok(render_body_html_or_htmx(StatusCode::OK, "Example form", form_example_body(
        Some("Content was valid".to_string()), None, FormExamplePayload::default(),
    ), htmx_context).into_response())
}

async fn not_found_handler(method: Method, uri: Uri, headers: HeaderMap) -> Result<Response, ResponseError> {
    Ok(render_body_html_or_htmx(StatusCode::OK, "Not found", html! {
        header {
            (render_nav_links())
            h1 { "Not Found" }
        }
        section {
            p {
                code { (method.as_str()) }
                " "
                code { (uri.path()) }
                " not found"
            }
        }
    }, HtmxContext::try_from(headers).ok()))
}

/// Wrap up the [HtmxContext] capture in a submodule.
mod htmx {
    use axum::http::HeaderMap;
    use anyhow::{anyhow, Error};
    use url::Url;
    use std::str::FromStr;

    #[derive(Debug,Default,Clone,PartialEq,Eq,PartialOrd,Ord)]
    pub struct HtmxContext {
        pub(crate) is_boost: bool,
        pub(crate) target: Option<String>,
        pub(crate) trigger: Option<String>,
        pub(crate) trigger_name: Option<String>,
        pub(crate) current_url: Option<Url>,
    }

    impl TryFrom<HeaderMap> for HtmxContext {
        type Error = Error;

        /// Capture the [HtmxContext] from the request headers. Although this is a "try" it should
        /// never fail in reality if coming from a well-behaved client. This should only fail if
        /// the client is badly behaved or someone is manually injecting headers. Then they get
        /// what they deserve (undefined client side behavior)
        fn try_from(value: HeaderMap) -> Result<Self, Self::Error> {
            if !value.contains_key("HX-Request") {
                Err(anyhow!("HX-Request header is missing"))?
            } else {
                let mut out = HtmxContext{
                    is_boost: value.get("HX-Boosted").is_some_and(|x| x.eq("true")),
                    ..HtmxContext::default()
                };
                if let Some(r) = value.get("HX-Target") {
                    out.target = Some(r.to_str()?.to_string());
                }
                if let Some(r) = value.get("HX-Trigger") {
                    out.trigger = Some(r.to_str()?.to_string());
                }
                if let Some(r) = value.get("HX-Trigger-Name") {
                    out.trigger_name = Some(r.to_str()?.to_string());
                }
                if let Some(r) = value.get("HX-Current-URL") {
                    out.current_url = Some(Url::from_str(r.to_str()?)?);
                }
                Ok(out)
            }
        }
    }
}