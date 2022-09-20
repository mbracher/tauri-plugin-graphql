// Copyright 2022 Jonas Kruckenberg
// SPDX-License-Identifier: MIT

//! This crate contains a Tauri plugin used to expose a [`juniper`] GraphQL
//! endpoint through Tauri's IPC system. This plugin can be used as safer
//! alternative to Tauri's existing Command API since both the Rust and
//! JavaScript side of the interface can be generated from a common schema.
//!
//! ## Rationale
//!
//! Especially in bigger projects that have specialized teams for the Frontend
//! and Rust core the existing command API falls short of being an optimal
//! solution. The Frontend is tightly coupled through `invoke()` calls to
//! backend commands, but there is no type-safety to alert Frontend developers
//! to changes in command signatures. This results in a very brittle interface
//! where changes on the Rust side will inadvertently break code in the
//! Frontend. This problem is similar exiting REST APIs, where the absence of a
//! formal contract between the server and the frontend makes future changes
//! very difficult.
//!
//! We can employ the same techniques used in traditional web development and
//! use shared schema that governs which types, methods, etc. are
//! available. GraphQL is such a schema language.
//!
//! ## Examples
//!
//! For the following examples, it is assumed you are familiar with [`Tauri
//! Commands`][`Commands`], [`Events`] and [`GraphQL`].
//!
//! ### Queries
//!
//! An example app that implements a very simple read-only todo-app using
//! GraphQL:
//!
//! ```rust
//! use async_graphql::{Schema, EmptySubscription, EmptyMutation, Object, SimpleObject, Result as GraphQLResult};
//!
//! #[derive(SimpleObject, Debug, Clone)]
//! struct ListItem {
//!     id: i32,
//!     text: String
//! }
//!
//! impl ListItem {
//!     pub fn new(text: String) -> Self {
//!         Self {
//!             id: rand::random::<i32>(),
//!             text
//!         }
//!     }
//! }
//!
//! struct Query;
//!
//! #[Object]
//! impl Query {
//!     async fn list(&self) -> GraphQLResult<Vec<ListItem>> {
//!         let item = vec![
//!             ListItem::new("foo".to_string()),
//!             ListItem::new("bar".to_string())
//!         ];
//!
//!         Ok(item)
//!     }
//! }
//!
//! let schema = Schema::new(
//!     Query,
//!     EmptyMutation,
//!     EmptySubscription,
//! );
//!
//! tauri::Builder::default()
//!     .plugin(tauri_plugin_graphql::init(schema));
//! ```
//!
//! ### Mutations
//!
//! GraphQL mutations provide a way to update or create state in the Core.
//!
//! Similarly to queries, mutations have access to a context object and can
//! manipulate windows, menus or global state.
//!
//! ```rust
//! use async_graphql::{Schema, Object, Context, EmptySubscription, EmptyMutation, SimpleObject, Result as GraphQLObject};
//! use tauri::{AppHandle, Manager};
//! use std::sync::Mutex;
//!
//! #[derive(Debug, Default)]
//! struct List(Mutex<Vec<ListItem>>);
//!
//! #[derive(SimpleObject, Debug, Clone)]
//! struct ListItem {
//!     id: i32,
//!     text: String
//! }
//!
//! impl ListItem {
//!     pub fn new(text: String) -> Self {
//!         Self {
//!             id: rand::random::<i32>(),
//!             text
//!         }
//!     }
//! }
//!
//! struct Query;
//!
//! #[Object]
//! impl Query {
//!     async fn list(&self, ctx: &Context<'_>) -> GraphQLObject<Vec<ListItem>> {
//!       let app = ctx.data::<AppHandle>().unwrap();
//!
//!       let list = app.state::<List>();
//!       let list = list.0.lock().unwrap();
//!         
//!       let items = list.iter().cloned().collect::<Vec<_>>();
//!
//!       Ok(items)
//!     }
//! }
//!
//! struct Mutation;
//!
//! #[Object]
//! impl Mutation {
//!   async fn add_entry(&self, ctx: &Context<'_>, text: String) -> GraphQLObject<ListItem> {
//!     let app = ctx.data::<AppHandle>().unwrap();
//!
//!     let list = app.state::<List>();
//!     let mut list = list.0.lock().unwrap();
//!
//!     let item = ListItem::new(text);
//!
//!     list.push(item.clone());
//!
//!     Ok(item)
//!   }
//! }
//!
//! let schema = Schema::new(
//!     Query,
//!     Mutation,
//!     EmptySubscription,
//! );
//!
//! tauri::Builder::default()
//!     .plugin(tauri_plugin_graphql::init(schema))
//!     .setup(|app| {
//!       app.manage(List::default());
//!
//!       Ok(())
//!     });
//! ```
//!
//! ### Subscriptions
//!
//! > **Support for GraphQL Subscriptions requires the `subscriptions` feature
//! flag**
//!
//! GraphQL subscriptions are a way to push real-time data to the Frontend.
//! Similarly to queries, a client can request a set of fields, but instead of
//! immediately returning a single answer, a new result is sent to the Frontend
//! every time the Core sends one.
//!
//! Subscription resolvers should be async and must return a [`Stream`].
//!
//! ```rust
//! use async_graphql::{futures_util::{self, stream::Stream}, Schema, Object, Subscription, EmptySubscription, EmptyMutation, SimpleObject, Result as GraphQLObject};
//!
//! struct Query;
//!
//! #[Object]
//! impl Query {
//!   async fn hello_world(&self) -> GraphQLObject<&str> {
//!     Ok("Hello World!")
//!   }
//! }
//!
//! struct Subscription;
//!
//! #[Subscription]
//! impl Subscription {
//!   async fn hello_world(&self) -> impl Stream<Item = &str> {
//!     futures_util::stream::iter(vec!["Hello", "World!"])
//!   }
//! }
//!
//! let schema = Schema::new(
//!   Query,
//!   EmptyMutation,
//!   Subscription,
//! );
//!
//! tauri::Builder::default()
//!   .plugin(tauri_plugin_graphql::init(schema));
//! ```
//!
//! ## Stability
//!
//! To work around limitations with the current command system, this plugin
//! directly implements an invoke handler instead of reyling on the
//! [`tauri::generate_handler`] macro.
//! Since the invoke handler implementation is not considered stable and might
//! change between releases **this plugin has no backwards compatibility
//! guarantees**.
//!
//! [`Stream`]: https://docs.rs/futures/latest/futures/stream/trait.Stream.html
//! [`Commands`]: https://tauri.studio/docs/guides/command
//! [`Events`]: https://tauri.studio/docs/guides/events
//! [`GraphQL`]: https://graphql.org

use std::sync::Arc;

use async_graphql::{
  futures_util::StreamExt, BatchRequest, ObjectType, Request, Schema, SubscriptionType,
};
use serde::Deserialize;
use tauri::{
  plugin::{self, TauriPlugin},
  InvokeError, Manager, Runtime,
};

/// Initializes the GraphQL plugin
///
/// This plugin exposes a juniper GraphQL endpoint via Tauri's IPC system,
/// allowing the frontend to invoke backend functionality through GraphQL.
/// **This does not open a web server.**
///
/// The `schema` argument must be a valid [`juniper::RootNode`].
///
/// ## Example
///
/// ```rust
/// use async_graphql::{Schema, Object, EmptyMutation, EmptySubscription, SimpleObject, Result as GraphQLResult};
///
/// #[derive(SimpleObject)]
/// struct User {
///     id: i32,
///     name: String
/// }
///
/// struct Query;
///
/// // Implement resolvers for all possible queries.
/// #[Object]
/// impl Query {
///     async fn me(&self) -> GraphQLResult<User> {
///         Ok(User {
///             id: 1,
///             name: "Luke Skywalker".to_string(),
///         })
///     }
/// }
///
/// let schema = Schema::new(
///     Query,
///     EmptyMutation,
///     EmptySubscription,
/// );
///
/// tauri::Builder::default()
///     .plugin(tauri_plugin_graphql::init(schema));
/// ```
pub fn init<R, Query, Mutation, Subscription>(
  schema: Schema<Query, Mutation, Subscription>,
) -> TauriPlugin<R>
where
  R: Runtime,
  Query: ObjectType + 'static,
  Mutation: ObjectType + 'static,
  Subscription: SubscriptionType + 'static,
{
  let schema = Arc::new(schema);

  plugin::Builder::new("graphql")
    .invoke_handler(move |invoke| {
      let window = invoke.message.window();

      let schema = schema.clone();

      match invoke.message.command() {
        "graphql" => invoke.resolver.respond_async(async move {
          let req: BatchRequest = serde_json::from_value(invoke.message.payload().clone())
            .map_err(InvokeError::from_serde_json)?;

          let resp = schema
            .execute_batch(req.data(window.app_handle()).data(window))
            .await;

          let str = serde_json::to_string(&resp).map_err(InvokeError::from_serde_json)?;

          Ok((str, resp.is_ok()))
        }),
        "subscriptions" => invoke.resolver.respond_async(async move {
          let req: SubscriptionRequest = serde_json::from_value(invoke.message.payload().clone())
            .map_err(InvokeError::from_serde_json)?;

          let subscription_window = window.clone();
          let mut stream = schema.execute_stream(req.inner.data(window.app_handle()).data(window));

          let event_id = &format!("graphql://{}", req.id);

          while let Some(result) = stream.next().await {
            let str = serde_json::to_string(&result).map_err(InvokeError::from_serde_json)?;

            subscription_window.emit(event_id, str)?;
          }
          subscription_window.emit(event_id, Option::<()>::None)?;

          Ok(())
        }),
        cmd => invoke.resolver.reject(format!(
          "Invalid endpoint \"{}\". Valid endpoints are: \"graphql\", \"subscriptions\".",
          cmd
        )),
      }
    })
    .build()
}

#[derive(Debug, Deserialize)]
pub struct SubscriptionRequest {
  #[serde(flatten)]
  inner: Request,
  id: u32,
}