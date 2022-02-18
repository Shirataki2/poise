//! A macro that generates backtracking-capable argument parsing code, given a list of parameter
//! types and attributes

#[doc(hidden)]
#[macro_export]
macro_rules! _parse_prefix {
    // All arguments have been consumed
    ( $ctx:ident $msg:ident $args:ident => [ $error:ident $( $name:ident )* ] ) => {
        if $args.is_empty() {
            return Ok(( $( $name, )* ));
        }
    };

    // Consume Option<T> greedy-first
    ( $ctx:ident $msg:ident $args:ident => [ $error:ident $($preamble:tt)* ]
        (Option<$type:ty $(,)?>)
        $( $rest:tt )*
    ) => {
        match $crate::pop_prefix_argument!($type, &$args, $ctx, $msg).await {
            Ok(($args, token)) => {
                let token: Option<$type> = Some(token);
                $crate::_parse_prefix!($ctx $msg $args => [ $error $($preamble)* token ] $($rest)* );
            },
            Err(e) => $error = e,
        }
        let token: Option<$type> = None;
        $crate::_parse_prefix!($ctx $msg $args => [ $error $($preamble)* token ] $($rest)* );
    };

    // Consume Option<T> lazy-first
    ( $ctx:ident $msg:ident $args:ident => [ $error:ident $($preamble:tt)* ]
        (#[lazy] Option<$type:ty $(,)?>)
        $( $rest:tt )*
    ) => {
        let token: Option<$type> = None;
        $crate::_parse_prefix!($ctx $msg $args => [ $error $($preamble)* token ] $($rest)* );
        match $crate::pop_prefix_argument!($type, &$args, $ctx, $msg).await {
            Ok(($args, token)) => {
                let token: Option<$type> = Some(token);
                $crate::_parse_prefix!($ctx $msg $args => [ $error $($preamble)* token ] $($rest)* );
            },
            Err(e) => $error = e,
        }
    };

    // Consume #[rest] Option<T> until the end of the input
    ( $ctx:ident $msg:ident $args:ident => [ $error:ident $($preamble:tt)* ]
        (#[rest] Option<$type:ty $(,)?>)
        $( $rest:tt )*
    ) => {
        if $args.trim_start().is_empty() {
            let token: Option<$type> = None;
            $crate::_parse_prefix!($ctx $msg $args => [ $error $($preamble)* token ]);
        } else {
            let input = $args.trim_start();
            match <$type as $crate::serenity_prelude::ArgumentConvert>::convert(
                $ctx, $msg.guild_id, Some($msg.channel_id), input
            ).await {
                Ok(token) => {
                    let $args = "";
                    let token = Some(token);
                    $crate::_parse_prefix!($ctx $msg $args => [ $error $($preamble)* token ]);
                },
                Err(e) => $error = (e.into(), Some(input.to_owned())),
            }
        }
    };

    // Consume Vec<T> greedy-first
    ( $ctx:ident $msg:ident $args:ident => [ $error:ident $($preamble:tt)* ]
        (Vec<$type:ty $(,)?>)
        $( $rest:tt )*
    ) => {
        let mut tokens = Vec::new();
        let mut token_rest_args = vec![$args.clone()];

        let mut running_args = $args.clone();
        loop {
            match $crate::pop_prefix_argument!($type, &running_args, $ctx, $msg).await {
                Ok((popped_args, token)) => {
                    tokens.push(token);
                    token_rest_args.push(popped_args.clone());
                    running_args = popped_args;
                },
                Err(e) => {
                    $error = e;
                    break;
                }

            }
        }

        // This will run at least once
        while let Some(token_rest_args) = token_rest_args.pop() {
            $crate::_parse_prefix!($ctx $msg token_rest_args => [ $error $($preamble)* tokens ] $($rest)* );
            tokens.pop();
        }
    };

    // deliberately no `#[rest] &str` here because &str isn't supported anywhere else and this
    // inconsistency and also the further implementation work makes it not worth it.

    // Consume #[rest] T as the last argument
    ( $ctx:ident $msg:ident $args:ident => [ $error:ident $($preamble:tt)* ]
        // question to my former self: why the $(poise::)* ?
        (#[rest] $(poise::)* $type:ty)
    ) => {
        let input = $args.trim_start();
        if input.is_empty() {
            $error = ($crate::TooFewArguments.into(), None);
        } else {
            match <$type as $crate::serenity_prelude::ArgumentConvert>::convert(
                $ctx, $msg.guild_id, Some($msg.channel_id), input
            ).await {
                Ok(token) => {
                    let $args = "";
                    $crate::_parse_prefix!($ctx $msg $args => [ $error $($preamble)* token ]);
                },
                Err(e) => $error = (e.into(), Some(input.to_owned())),
            }
        }
    };

    // Consume #[flag] FLAGNAME
    ( $ctx:ident $msg:ident $args:ident => [ $error:ident $($preamble:tt)* ]
        (#[flag] $name:literal)
        $( $rest:tt )*
    ) => {
        match $crate::pop_prefix_argument!(String, &$args, $ctx, $msg).await {
            Ok(($args, token)) if token.eq_ignore_ascii_case($name) => {
                $crate::_parse_prefix!($ctx $msg $args => [ $error $($preamble)* true ] $($rest)* );
            },
            // only allow backtracking if the flag didn't match: it's confusing for the user if they
            // precisely set the flag but it's ignored
            _ => {
                $error = (concat!("Must use either `", $name, "` or nothing as a modifier").into(), None);
                $crate::_parse_prefix!($ctx $msg $args => [ $error $($preamble)* false ] $($rest)* );
            }
        }
    };

    // Consume T
    ( $ctx:ident $msg:ident $args:ident => [ $error:ident $($preamble:tt)* ]
        ($type:ty)
        $( $rest:tt )*
    ) => {
        match $crate::pop_prefix_argument!($type, &$args, $ctx, $msg).await {
            Ok(($args, token)) => {
                $crate::_parse_prefix!($ctx $msg $args => [ $error $($preamble)* token ] $($rest)* );
            },
            Err(e) => $error = e,
        }
    };

    // ( $($t:tt)* ) => {
    //     compile_error!( stringify!($($t)*) );
    // };
}

/**
Macro for parsing an argument string into multiple parameter types.

An invocation of this macro is generated by the [`crate::command`] macro, so you usually don't need
to use this macro directly.

```rust
# #[tokio::main] async fn main() -> Result<(), Box<dyn std::error::Error>> {
# use poise::serenity_prelude as serenity;
# let ctx = serenity::Context {
#     data: std::sync::Arc::new(serenity::RwLock::new(serenity::TypeMap::new())),
#     shard: ::serenity::client::bridge::gateway::ShardMessenger::new(
#         futures::channel::mpsc::unbounded().0,
#     ),
#     shard_id: Default::default(),
#     http: Default::default(),
#     cache: Default::default(),
# };
# let msg = serenity::CustomMessage::new().build();

assert_eq!(
    poise::parse_prefix_args!(
        &ctx, &msg,
        "one two three four" => (String), (Option<u32>), #[rest] (String)
    ).await.unwrap(),
    (
        String::from("one"),
        None,
        String::from("two three four"),
    ),
);

assert_eq!(
    poise::parse_prefix_args!(
        &ctx, &msg,
        "1 2 3 4" => (String), (Option<u32>), #[rest] (String)
    ).await.unwrap(),
    (
        String::from("1"),
        Some(2),
        String::from("3 4"),
    ),
);

# Ok(()) }
```
*/
#[macro_export]
macro_rules! parse_prefix_args {
    ($ctx:expr, $msg:expr, $args:expr => $(
        $( #[$attr:ident] )?
        ( $($type:tt)* )
    ),* $(,)? ) => {
        async {
            use $crate::PopArgument as _;

            let ctx = $ctx;
            let msg = $msg;
            let args = $args;

            let mut error: (Box<dyn std::error::Error + Send + Sync>, Option<String>)
                = (Box::new($crate::TooManyArguments) as _, None);

            $crate::_parse_prefix!(
                ctx msg args => [error]
                $(
                    ($( #[$attr] )? $($type)*)
                )*
            );
            Err(error)
        }
    };
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_parse_args() {
        use crate::serenity_prelude as serenity;

        // Create dummy discord context; it will not be accessed in this test
        let ctx = serenity::Context {
            data: std::sync::Arc::new(serenity::RwLock::new(serenity::TypeMap::new())),
            shard: ::serenity::client::bridge::gateway::ShardMessenger::new(
                futures::channel::mpsc::unbounded().0,
            ),
            shard_id: Default::default(),
            http: Default::default(),
            cache: Default::default(),
        };
        let msg = serenity::CustomMessage::new().build();

        assert_eq!(
            parse_prefix_args!(&ctx, &msg, "hello" => (Option<String>), (String))
                .await
                .unwrap(),
            (None, "hello".into()),
        );
        assert_eq!(
            parse_prefix_args!(&ctx, &msg, "a b c" => (Vec<String>), (String))
                .await
                .unwrap(),
            (vec!["a".into(), "b".into()], "c".into()),
        );
        assert_eq!(
            parse_prefix_args!(&ctx, &msg, "a b c" => (Vec<String>), (Vec<String>))
                .await
                .unwrap(),
            (vec!["a".into(), "b".into(), "c".into()], vec![]),
        );
        assert_eq!(
            parse_prefix_args!(&ctx, &msg, "a b 8 c" => (Vec<String>), (u32), (Vec<String>))
                .await
                .unwrap(),
            (vec!["a".into(), "b".into()], 8, vec!["c".into()]),
        );
        assert_eq!(
            parse_prefix_args!(&ctx, &msg, "yoo `that's cool` !" => (String), (crate::CodeBlock), (String))
                .await
                .unwrap(),
            (
                "yoo".into(),
                crate::CodeBlock {
                    code: "that's cool".into(),
                    language: None
                },
                "!".into()
            ),
        );
        assert_eq!(
            parse_prefix_args!(&ctx, &msg, "hi" => #[lazy] (Option<String>), (Option<String>))
                .await
                .unwrap(),
            (None, Some("hi".into())),
        );
        assert_eq!(
            parse_prefix_args!(&ctx, &msg, "a b c" => (String), #[rest] (String))
                .await
                .unwrap(),
            ("a".into(), "b c".into()),
        );
        assert_eq!(
            parse_prefix_args!(&ctx, &msg, "a b c" => (String), #[rest] (String))
                .await
                .unwrap(),
            ("a".into(), "b c".into()),
        );
        assert!(
            parse_prefix_args!(&ctx, &msg, "hello" => #[flag] ("hello"), #[rest] (String))
                .await
                .unwrap_err()
                .0
                .is::<crate::TooFewArguments>(),
        );
        assert_eq!(
            parse_prefix_args!(&ctx, &msg, "helloo" => #[flag] ("hello"), #[rest] (String))
                .await
                .unwrap(),
            (false, "helloo".into())
        );
    }
}
