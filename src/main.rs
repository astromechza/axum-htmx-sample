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
        .route("/favicon.svg", get(favicon_svg_handler))
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
            main class="container" {
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
                style {
                    r#"
                        /**
                         * Minified by jsDelivr using clean-css v5.3.2.
                         * Original file: /npm/modern-normalize@3.0.1/modern-normalize.css
                         *
                         * Do NOT use SRI with dynamically generated files! More information: https://www.jsdelivr.com/using-sri-with-dynamic-files
                         */
                        /*! modern-normalize v3.0.1 | MIT License | https://github.com/sindresorhus/modern-normalize */
                        *,::after,::before{box-sizing:border-box}html{font-family:system-ui,'Segoe UI',Roboto,Helvetica,Arial,sans-serif,'Apple Color Emoji','Segoe UI Emoji';line-height:1.15;-webkit-text-size-adjust:100%;tab-size:4}body{margin:0}b,strong{font-weight:bolder}code,kbd,pre,samp{font-family:ui-monospace,SFMono-Regular,Consolas,'Liberation Mono',Menlo,monospace;font-size:1em}small{font-size:80%}sub,sup{font-size:75%;line-height:0;position:relative;vertical-align:baseline}sub{bottom:-.25em}sup{top:-.5em}table{border-color:currentcolor}button,input,optgroup,select,textarea{font-family:inherit;font-size:100%;line-height:1.15;margin:0}[type=button],[type=reset],[type=submit],button{-webkit-appearance:button}legend{padding:0}progress{vertical-align:baseline}::-webkit-inner-spin-button,::-webkit-outer-spin-button{height:auto}[type=search]{-webkit-appearance:textfield;outline-offset:-2px}::-webkit-search-decoration{-webkit-appearance:none}::-webkit-file-upload-button{-webkit-appearance:button;font:inherit}summary{display:list-item}
                        /*# sourceMappingURL=/sm/d2d8cd206fb9f42f071e97460f3ad9c875edb5e7a4b10f900a83cdf8401c53a9.map */
                    "#
                }
                link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/milligram/1.4.1/milligram.min.css" integrity="sha512-xiunq9hpKsIcz42zt0o2vCo34xV0j6Ny8hgEylN3XBglZDtTZ2nwnqF/Z/TTCc18sGdvCjbFInNd++6q3J0N6g==" crossorigin="anonymous" referrerpolicy="no-referrer";
                link rel="shortcut icon" type="image/svg" href="/favicon.svg";
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
    hm.insert("Vary", HeaderValue::from_static("HX-Request"));
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
        main class="container" {
            header {
                (render_nav_links())
                h1 { "Home" }
            }
            section {
                p {
                    "This is the home page."
                }
            }
        }
    }, HtmxContext::try_from(headers).ok()).into_response())
}

async fn favicon_svg_handler() -> Result<Response, ResponseError> {
    let mut hm = HeaderMap::new();
    hm.insert("Content-Type", HeaderValue::from_static("image/svg+xml"));
    Ok((StatusCode::OK, hm, r#"
    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
      <rect width="100" height="100" fill="black"/>
    </svg>
    "#).into_response())
}

async fn fallible_handler(headers: HeaderMap) -> Result<Response, ResponseError> {
    let htmx_context = HtmxContext::try_from(headers).ok();

    // Produce an error response sometimes.
    if rand::random::<bool>() {
        Err(anyhow!("request was unlucky")).map_resp_err(htmx_context.clone())?
    }

    Ok(render_body_html_or_htmx(StatusCode::OK, "Lucky!", html! {
        main class="container" {
            header {
                (render_nav_links())
                h1 { "Lucky you" }
            }
            section {
                p {
                    "You were lucky!"
                }
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
        main class="container" {
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
    let accept_html = headers.get("Accept")
        .and_then(|raw| raw.to_str().ok().map(|ct| ct.contains("text/html") || ct.contains("*/*")))
        .unwrap_or(true);
    if !accept_html {
        return Ok(StatusCode::NOT_FOUND.into_response());
    }

    Ok(render_body_html_or_htmx(StatusCode::OK, "Not found", html! {
        main class="container" {
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