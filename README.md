# `wahoo`

Wahoo is a dynamic site generator based on the [Tera](https://crates.io/crates/tera) templating engine.

Wahoo allows you to create custom static site generation using tera templates and TOML configuration files, the data from which is available during the template rendering.

## Overview

In a number of ways, Wahoo provides workflow similar to [mdBook](https://rust-lang.github.io/mdBook/) - you can edit the site while observing the real-time changes to the site in the browser.

## Features 

- Unstructured TOML configuration files data from which is available to tera templates during rendering.
- `serve` mode with an automatic re-rendering of the content and page updates triggered by file changes.
- Support for full-page or partial markdown content.

Documentation for the Tera templating engine is available at https://tera.netlify.app/docs

