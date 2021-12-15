use crate::serenity_prelude as serenity;
use crate::BoxFuture;

/// A builder to configure and run a framework.
///
/// If one of the following required values is missing, the builder will panic on start:
/// - [`Self::token`]
/// - [`Self::user_data_setup`]
/// - [`Self::options`]
///
/// Before starting, the builder will make an HTTP request to retrieve the bot's application ID and
/// owner.
pub struct FrameworkBuilder<U, E> {
    user_data_setup: Option<
        Box<
            dyn Send
                + Sync
                + for<'a> FnOnce(
                    &'a serenity::Context,
                    &'a serenity::Ready,
                    &'a crate::Framework<U, E>,
                ) -> BoxFuture<'a, Result<U, E>>,
        >,
    >,
    options: Option<crate::FrameworkOptions<U, E>>,
    client_settings:
        Option<Box<dyn FnOnce(serenity::ClientBuilder) -> serenity::ClientBuilder>>,
    token: Option<String>,
    intents: Option<serenity::GatewayIntents>,
    commands: Vec<(
        crate::CommandDefinition<U, E>,
        Box<dyn FnOnce(&mut crate::CommandBuilder<U, E>) -> &mut crate::CommandBuilder<U, E>>,
    )>,
}

impl<U, E> Default for FrameworkBuilder<U, E> {
    fn default() -> Self {
        Self {
            user_data_setup: Default::default(),
            options: Default::default(),
            client_settings: Default::default(),
            token: Default::default(),
            intents: Default::default(),
            commands: Default::default(),
        }
    }
}

impl<U, E> FrameworkBuilder<U, E> {
    /// Set a prefix for commands
    #[deprecated = "Please set the prefix via FrameworkOptions::prefix_options::prefix"]
    pub fn prefix(self, _prefix: impl Into<String>) -> Self {
        panic!("Please set the prefix via FrameworkOptions::prefix_options::prefix");
    }

    /// Set a callback to be invoked to create the user data instance
    pub fn user_data_setup<F>(mut self, user_data_setup: F) -> Self
    where
        F: Send
            + Sync
            + 'static
            + for<'a> FnOnce(
                &'a serenity::Context,
                &'a serenity::Ready,
                &'a crate::Framework<U, E>,
            ) -> BoxFuture<'a, Result<U, E>>,
    {
        self.user_data_setup = Some(Box::new(user_data_setup) as _);
        self
    }

    /// Configure framework options
    pub fn options(mut self, options: crate::FrameworkOptions<U, E>) -> Self {
        self.options = Some(options);
        self
    }

    /// Configure serenity client settings, like gateway intents, by supplying a custom
    /// client builder
    ///
    /// Note: the builder's token will be overridden by the
    /// [`FrameworkBuilder`]; use [`FrameworkBuilder::token`] to supply a token.
    pub fn client_settings(
        mut self,
        f: impl FnOnce(serenity::ClientBuilder) -> serenity::ClientBuilder + 'static,
    ) -> Self {
        self.client_settings = Some(Box::new(f));
        self
    }

    /// The bot token
    pub fn token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    /// Add a new command to the framework
    pub fn command(
        mut self,
        definition: crate::CommandDefinition<U, E>,
        meta_builder: impl FnOnce(&mut crate::CommandBuilder<U, E>) -> &mut crate::CommandBuilder<U, E>
            + 'static,
    ) -> Self {
        self.commands.push((definition, Box::new(meta_builder)));
        self
    }

    /// Add multiple new commands to the framework. Shorthand for calling [`Self::command`] multiple
    /// times with the builder left to defaults, i.e. no command category or subcommands
    ///
    /// ```rust
    /// # type Error = Box<dyn std::error::Error + Send + Sync>;
    /// # #[poise::command(prefix_command)]
    /// # async fn command1(ctx: poise::Context<'_, (), Error>) -> Result<(), Error> { Ok(()) }
    /// # #[poise::command(prefix_command)]
    /// # async fn command2(ctx: poise::Context<'_, (), Error>) -> Result<(), Error> { Ok(()) }
    ///
    /// poise::Framework::build()
    ///     // framework setup...
    ///     .commands([command1, command2])
    ///     // framework startup...
    /// # ;
    /// ```
    pub fn commands(
        mut self,
        commands: impl IntoIterator<Item = fn() -> crate::CommandDefinition<U, E>> + 'static,
    ) -> Self {
        // Can't use Vec::extend() due to ??? compile errors
        for command in commands {
            let definition = (command)();
            self.commands.push((definition, Box::new(|f| f)));
        }
        self
    }

    /// Build the framework with the specified configuration.
    ///
    /// For more information, see [`FrameworkBuilder`]
    pub async fn build(self) -> Result<std::sync::Arc<crate::Framework<U, E>>, serenity::Error>
    where
        U: Send + Sync + 'static,
        E: Send + 'static,
    {
        // Aggregate required values or panic if not provided
        let token = self.token.expect("No token was provided to the framework");
        let user_data_setup = self
            .user_data_setup
            .expect("No user data setup function was provided to the framework");
        let mut options = self.options.expect("No framework options provided");

        // Retrieve application info via HTTP
        let application_info = serenity::Http::new_with_token(&token)
            .get_current_application_info()
            .await?;

        // Build framework options by concatenating user-set options with commands and owner
        for (command, meta_builder) in self.commands {
            options.command(command, meta_builder);
        }
        options.owners.insert(application_info.owner.id);

        // Create serenity client
        let mut client_builder = serenity::ClientBuilder::new(token)
            .application_id(application_info.id.0)
            .intents(
                self.intents
                    .unwrap_or_else(serenity::GatewayIntents::non_privileged),
            );
        if let Some(client_settings) = self.client_settings {
            client_builder = client_settings(client_builder);
        }

        // Create framework with specified settings
        crate::Framework::new(
            serenity::ApplicationId(application_info.id.0),
            client_builder,
            user_data_setup,
            options,
        )
        .await
    }

    /// Start the framework with the specified configuration.
    ///
    /// [`FrameworkBuilder::run`] is just a shorthand that calls [`FrameworkBuilder::build`] and
    /// starts the returned framework
    pub async fn run(self) -> Result<(), serenity::Error>
    where
        U: Send + Sync + 'static,
        E: Send + 'static,
    {
        self.build().await?.start().await
    }
}
