# axum-htmx-sample

A sample in Rust using axum and htmx to reduce bytes transferred for simpler html sites with progressive enhancement.

Although HTMX _can_ be much more complicated, this example works on a few basic principles:

1. Use `hx-boost="true"` by default. This applies to all the GET links and POST form and simplifies their definition.
2. The POST form demonstrates validation and success messages in a simple way which works fine for the basic use-case
   but can be more complicated with partial out-of-band swaps if the page is expensive to render in the future.
3. Progressive enhancement that works with javascript disabled entirely.

Ideas for further work:

1. Inline the favicon and CSS styles into the head of the page so that it is only fetched with the initial load and
    easily avoided by leveraging HTMX. Naturally the HTMX library is a bit large for inlining, but owned assets like
    CSS or inline scripts may be suitable.
2. Demonstrate a table-based pagination.
