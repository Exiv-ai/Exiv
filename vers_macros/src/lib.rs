use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, LitStr, parse::Parse, parse::ParseStream, Token, Ident, ExprArray, Expr};
use base64::{Engine as _, engine::general_purpose};
use std::path::PathBuf;

struct PluginAttr {
    name: String,
    service_type: String,
    description: String,
    version: String,
    icon: Option<String>,
    action_icon: Option<String>,
    config_keys: Vec<String>,
    permissions: Vec<String>,
    capabilities: Vec<String>,
}

impl Parse for PluginAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut name = String::new();
        let mut service_type = String::new();
        let mut description = String::new();
        let mut version = String::from("0.1.0");
        let mut icon = None;
        let mut action_icon = None;
        let mut config_keys = Vec::new();
        let mut permissions = Vec::new();
        let mut capabilities = Vec::new();

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            
            if input.peek(LitStr) {
                let val: LitStr = input.parse()?;
                match key.to_string().as_str() {
                    "name" => name = val.value(),
                    "kind" => service_type = val.value(),
                    "description" => description = val.value(),
                    "version" => version = val.value(),
                    "icon" => icon = Some(val.value()),
                    "action_icon" => action_icon = Some(val.value()),
                    _ => {}
                }
            } else if input.peek(syn::token::Bracket) {
                let content: ExprArray = input.parse()?;
                let vals: Vec<String> = content.elems.iter().filter_map(|e| {
                    if let Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) = e {
                        Some(s.value())
                    } else {
                        None
                    }
                }).collect();

                match key.to_string().as_str() {
                    "permissions" => permissions = vals,
                    "capabilities" => capabilities = vals,
                    "config_keys" => config_keys = vals,
                    _ => {}
                }
            }
            
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(PluginAttr { name, service_type, description, version, icon, action_icon, config_keys, permissions, capabilities })
    }
}

#[proc_macro_attribute]
pub fn vers_plugin(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let attr = parse_macro_input!(attr as PluginAttr);
    
    let name = &input.ident;
    let factory_name = quote::format_ident!("{}Factory", name);
    let plugin_name_str = &attr.name;
    let service_type_ident = quote::format_ident!("{}", attr.service_type);
    let description_str = &attr.description;
    let version_str = &attr.version;
    let action_icon_token = match attr.action_icon {
        Some(i) => quote! { Some(#i.to_string()) },
        None => quote! { None },
    };
    let config_keys_tokens = attr.config_keys.iter().map(|k| quote! { #k.to_string() });

    // Handle Icon embedding
    let mut icon_base64 = None;
    if let Some(ref icon_path_str) = attr.icon {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
        let mut icon_path = PathBuf::from(manifest_dir);
        icon_path.push(icon_path_str);

        match std::fs::read(&icon_path) {
            Ok(bytes) => {
                if bytes.len() > 64 * 1024 {
                    return syn::Error::new_spanned(
                        &input.ident,
                        format!("🔌 Plugin Icon '{}' is too large ({} bytes). Limit is 64KB.", icon_path_str, bytes.len())
                    ).to_compile_error().into();
                }
                icon_base64 = Some(general_purpose::STANDARD.encode(bytes));
            }
            Err(e) => {
                return syn::Error::new_spanned(
                    &input.ident,
                    format!("🔌 Failed to read icon at '{}': {}", icon_path.display(), e)
                ).to_compile_error().into();
            }
        }
    }

    let icon_data_tokens = match icon_base64 {
        Some(data) => quote! { Some(#data.to_string()) },
        None => quote! { None },
    };

    let perms = attr.permissions.iter().map(|p| {
        let ident = quote::format_ident!("{}", p);
        quote! { vers_shared::Permission::#ident }
    });

    let caps = attr.capabilities.iter().map(|c| {
        let ident = quote::format_ident!("{}", c);
        quote! { vers_shared::CapabilityType::#ident }
    });

    let default_tags = match attr.service_type.as_str() {
        "Reasoning" => vec!["#MIND", "#LLM"],
        "Memory" => vec!["#MEMORY"],
        "Skill" | "Action" => vec!["#TOOL"],
        "HAL" => vec!["#HAL"],
        "Communication" => vec!["#ADAPTER"],
        _ => vec![],
    };
    let tags_tokens = default_tags.iter().map(|t| quote! { #t.to_string() });

    // Auto-implement PluginCast based on capabilities
    let mut cast_methods = quote! {};
    for cap in &attr.capabilities {
        match cap.as_str() {
            "Reasoning" => {
                cast_methods = quote! {
                    #cast_methods
                    fn as_reasoning(&self) -> Option<&dyn vers_shared::ReasoningEngine> { Some(self) }
                };
            },
            "Memory" => {
                cast_methods = quote! {
                    #cast_methods
                    fn as_memory(&self) -> Option<&dyn vers_shared::MemoryProvider> { Some(self) }
                };
            },
            "Communication" => {
                cast_methods = quote! {
                    #cast_methods
                    fn as_communication(&self) -> Option<&dyn vers_shared::CommunicationAdapter> { Some(self) }
                };
            },
            "Tool" => {
                cast_methods = quote! {
                    #cast_methods
                    fn as_tool(&self) -> Option<&dyn vers_shared::Tool> { Some(self) }
                };
            },
            "Web" => {
                cast_methods = quote! {
                    #cast_methods
                    fn as_web(&self) -> Option<&dyn vers_shared::WebPlugin> { Some(self) }
                };
            },
            _ => {} // Vision, HAL, etc. do not have specific traits yet
        }
    }

    let expanded = quote! {
        #input

        impl vers_shared::PluginCast for #name {
            fn as_any(&self) -> &dyn std::any::Any { self }
            #cast_methods
        }

        pub struct #factory_name;

        #[async_trait::async_trait]
        impl vers_shared::PluginFactory for #factory_name {
            fn name(&self) -> &str { #plugin_name_str }
            fn service_type(&self) -> vers_shared::ServiceType {
                vers_shared::ServiceType::#service_type_ident
            }
            async fn create(&self, config: vers_shared::PluginConfig) -> anyhow::Result<std::sync::Arc<dyn vers_shared::Plugin>> {
                let plugin = #name::new_plugin(config).await?;
                Ok(std::sync::Arc::new(plugin))
            }
        }

        impl #name {
            pub fn factory() -> std::sync::Arc<dyn vers_shared::PluginFactory> {
                std::sync::Arc::new(#factory_name)
            }

            fn auto_manifest(&self) -> vers_shared::PluginManifest {
                vers_shared::PluginManifest {
                    id: #plugin_name_str.to_string(),
                    name: #plugin_name_str.to_string(),
                    description: #description_str.to_string(),
                    version: #version_str.to_string(),
                    service_type: vers_shared::ServiceType::#service_type_ident,
                    tags: vec![ #(#tags_tokens),* ],
                    is_active: true,
                    is_configured: true,
                    required_config_keys: vec![ #(#config_keys_tokens),* ],
                    action_icon: #action_icon_token,
                    action_target: None,
                    icon_data: #icon_data_tokens,
                    magic_seal: 0x56455253, // VERS
                    sdk_version: env!("CARGO_PKG_VERSION").to_string(),
                    required_permissions: vec![ #(#perms),* ],
                    provided_capabilities: vec![ #(#caps),* ],
                    provided_tools: vec![],
                }
            }
        }

        vers_shared::inventory::submit! {
            vers_shared::PluginRegistrar {
                factory: #name::factory,
            }
        }
    };

    TokenStream::from(expanded)
}