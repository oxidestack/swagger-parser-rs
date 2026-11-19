//! swagger-compat-spec-parser 1.0.76: `SwaggerCompatConverter`, which reads Swagger 1.x resource listings
//! and API declarations (after its JSON-patch based 1.1 → 1.2 migrations) and converts them to a Swagger
//! 2.0 model. It is one of the Swagger 1.x `SwaggerParserExtension`s the v2 converter's `SwaggerParser`
//! consults.

mod migrate;
mod model;

use crate::core::AuthorizationValue;
use crate::java::path::JavaPath;
use crate::java::{JMap, JResult, JavaException, MapKind};
use crate::v2::models as v2;
use crate::v2::models::{Model as V2Model, ModelKind, Parameter as V2Parameter, ParameterKind, Property, PropertyKind, PropertyRef};
use crate::v2::parser::{read_json, read_location_text, Swagger20Parser};
use crate::v2::property_builder::{self, Arg, Args, PropertyId};
use indexmap::IndexMap;
use jackson::{BigDecimal, JsonNode};
use model::*;

fn npe() -> JavaException {
    JavaException::without_message("java.lang.NullPointerException")
}

/// Jackson's failure to write a `null` map key (the Java code builds such maps and fails when serializing)
fn npe_msg(m: &str) -> JavaException {
    JavaException::npe(m)
}

/// `new GenericRef(type, null, INTERNAL)`
fn null_ref() -> JavaException {
    npe_msg("Cannot invoke \"String.startsWith(String)\" because \"ref\" is null")
}

fn null_key() -> JavaException {
    JavaException::illegal_argument("Null key for a Map not allowed in JSON (use a converting NullKeySerializer?)")
}

/// `io.swagger.parser.SwaggerCompatConverter`
#[derive(Default)]
pub struct SwaggerCompatConverter;

impl SwaggerCompatConverter {
    /// `new SwaggerCompatConverter()`.
    pub fn new() -> SwaggerCompatConverter {
        SwaggerCompatConverter
    }

    /// `readWithInfo(location, auths)`: the converted document serialized by swagger-core 1.x and parsed
    /// again by `Swagger20Parser`; an empty result for anything that is not a Swagger 1.x listing.
    pub fn read_with_info(&self, location: &str, auths: &[AuthorizationValue]) -> JResult<v2::SwaggerDeserializationResult> {
        let Some(swagger) = self.read(location, auths)? else { return Ok(v2::SwaggerDeserializationResult::default()) };
        let node = crate::v2::json::swagger_to_tree_v1(&swagger).map_err(JavaException::from)?;
        Swagger20Parser.read_with_info_node(&node)
    }

    /// `read(input, auths)`
    pub fn read(&self, input: &str, auths: &[AuthorizationValue]) -> JResult<Option<v2::Swagger>> {
        let Some(listing) = self.read_resource_listing(input, auths) else { return Ok(None) };
        let mut apis: Vec<ApiDeclaration> = Vec::new();
        let mut read_as_single_file = false;
        for r in listing.apis.clone().unwrap_or_default() {
            let r = r.ok_or_else(npe)?;
            let mut declaration: Option<ApiDeclaration> = None;
            if r.extra.get("operations").is_some() {
                if !read_as_single_file {
                    declaration = self.read_declaration(input, auths);
                    read_as_single_file = true;
                }
            } else {
                let ref_path = r.path.clone().ok_or_else(npe)?;
                let mut location = if input.starts_with("http") {
                    if ref_path.starts_with("http") {
                        ref_path.clone()
                    } else if ref_path.starts_with('/') {
                        if listing.swagger_version == Some("1.1") && listing.extra.get("basePath").is_some() {
                            let base = listing.extra.get("basePath").unwrap().text_value().map(str::to_string);
                            format!("{}{}", base.unwrap_or_else(|| "null".into()), ref_path)
                        } else {
                            format!("{input}{ref_path}")
                        }
                    } else {
                        format!("{input}/{ref_path}")
                    }
                } else {
                    let parent = JavaPath::new(input).get_parent().map(|p| p.to_string()).unwrap_or_else(|| "null".into());
                    if ref_path.starts_with('/') { format!("{parent}{ref_path}") } else { format!("{parent}/{ref_path}") }
                };
                if location.contains(".{format}") {
                    location = location.replace(".{format}", ".json");
                }
                declaration = self.read_declaration(&location, auths);
            }
            if let Some(d) = declaration {
                apis.push(d);
            }
        }
        self.convert(&listing, &apis).map(Some)
    }

    fn read_json_at(&self, input: &str, auths: &[AuthorizationValue]) -> JResult<JsonNode> {
        let json = read_location_text(input, auths)?;
        read_json(&json)
    }

    /// `readResourceListing(input, messages, auths)`: every failure yields null.
    fn read_resource_listing(&self, input: &str, auths: &[AuthorizationValue]) -> Option<ResourceListing> {
        let attempt = || -> JResult<Option<ResourceListing>> {
            let node = self.read_json_at(input, auths)?;
            if node.get("swaggerVersion").is_none() {
                return Ok(None);
            }
            let transformed = migrate::run(&node, migrate::resource_listing)?;
            resource_listing(&transformed)
        };
        attempt().ok().flatten()
    }

    /// `readDeclaration(input, messages, auths)`: every failure yields null.
    fn read_declaration(&self, input: &str, auths: &[AuthorizationValue]) -> Option<ApiDeclaration> {
        let attempt = || -> JResult<Option<ApiDeclaration>> {
            let mut node = self.read_json_at(input, auths)?;
            if let Some(o) = node.as_object_mut() {
                o.shift_remove("authorizations");
            }
            if node.is_missing() {
                return Err(npe());
            }
            let transformed = migrate::run(&node, migrate::api_declaration)?;
            api_declaration(&transformed)
        };
        attempt().ok().flatten()
    }

    fn convert_model(&self, m: &Model12) -> JResult<v2::ModelRef> {
        let mut out = V2Model::new(ModelKind::Impl);
        out.name = m.id.clone();
        out.set_description(m.description.clone());
        out.discriminator = m.discriminator.clone();
        if let Some(r) = &m.required {
            out.set_required(Some(r.clone()));
        }
        let props = m.properties.clone().ok_or_else(|| npe_msg("Cannot invoke \"java.util.Map.keySet()\" because the return value of \"io.swagger.models.apideclaration.Model.getProperties()\" is null"))?;
        for (k, p) in props {
            let p = p.ok_or_else(npe)?;
            let prop = self.property_from_typed(&p.typed)?.ok_or_else(|| npe_msg("Cannot invoke \"io.swagger.models.properties.Property.setDescription(String)\" because \"output\" is null"))?;
            prop.borrow_mut().description = p.description.clone();
            out.add_property(&k, Some(prop));
        }
        Ok(out.into_ref())
    }

    fn convert_parameter(&self, p: &Parameter12) -> JResult<v2::ParameterRef> {
        let kind = match p.param_type {
            Some("path") => ParameterKind::Path,
            Some("query") => ParameterKind::Query,
            Some("header") => ParameterKind::Header,
            Some("body") => ParameterKind::Body,
            Some("form") => ParameterKind::Form,
            _ => return Err(npe_msg("Cannot invoke \"io.swagger.models.parameters.Parameter.setName(String)\" because \"output\" is null")),
        };
        let mut out = V2Parameter::new(kind);
        if kind != ParameterKind::Body {
            out.default_value = p.typed.default_value.clone();
            out.enum_ = p.typed.enum_values.clone();
        }
        out.name = p.name.clone();
        out.description = p.description.clone();
        if let Some(r) = p.required {
            out.required = r;
        }
        let ty = p.typed.type_.clone();
        let format = p.typed.format.map(str::to_string);
        if kind == ParameterKind::Body {
            out.schema = self.model_from_typed(&p.typed)?;
        } else {
            let prop: PropertyRef = if p.allow_multiple == Some(true) {
                let mut ap = Property::new(PropertyKind::Array);
                ap.items = property_builder::build(ty.as_deref(), format.as_deref(), &Args::new())?.map(Property::into_ref);
                ap.into_ref()
            } else {
                match self.property_from_typed(&p.typed)? {
                    Some(x) => x,
                    None => Property::new(PropertyKind::String).into_ref(),
                }
            };
            let pb = prop.borrow();
            if pb.kind == PropertyKind::Array {
                out.set_type(Some("array".into()));
                out.collection_format = Some("csv".into());
                out.items = pb.items.clone();
            } else {
                out.set_type(pb.type_.clone());
                out.format = pb.format.clone();
            }
        }
        if kind == ParameterKind::Path {
            out.required = true;
        }
        Ok(out.into_ref())
    }

    /// `modelFromExtendedTypedObject(obj)`
    fn model_from_typed(&self, obj: &Typed) -> JResult<Option<v2::ModelRef>> {
        let ty = obj.type_.clone();
        let format = obj.format.map(str::to_string);
        if let Some(r) = &obj.ref_ {
            return Ok(Some(V2Model::new_ref(&format!("#/definitions/{r}"), None).into_ref()));
        }
        if ty.as_deref() == Some("array") {
            let mut am = V2Model::new(ModelKind::Array);
            let items = obj.items.clone().ok_or_else(|| npe_msg("Cannot invoke \"io.swagger.models.apideclaration.Items.getType()\" because \"items\" is null"))?;
            let t = items.type_.clone();
            let f = items.format.map(str::to_string);
            if let Some(inner) = property_builder::build(t.as_deref(), f.as_deref(), &Args::new())? {
                am.items = Some(inner.into_ref());
            }
            am.items = Some(match &items.ref_ {
                Some(r) => Property::new_ref(r, Some(v2::RefFormat::Internal)).into_ref(),
                None => Property::new_ref(t.as_deref().ok_or_else(null_ref)?, Some(v2::RefFormat::Internal)).into_ref(),
            });
            return Ok(Some(am.into_ref()));
        }
        match property_builder::build(ty.as_deref(), format.as_deref(), &Args::new())? {
            Some(input) => Ok(to_model(&input)),
            None => {
                if ty.as_deref() != Some("void") {
                    Ok(Some(V2Model::new_ref(&format!("#/definitions/{}", ty.as_deref().unwrap_or("null")), None).into_ref()))
                } else {
                    Ok(None)
                }
            }
        }
    }

    /// `propertyFromTypedObject(obj)`
    fn property_from_typed(&self, obj: &Typed) -> JResult<Option<PropertyRef>> {
        let ty = obj.type_.clone();
        let format = obj.format.map(str::to_string);
        if ty.as_deref() == Some("array") {
            let mut am = Property::new(PropertyKind::Array);
            let items = obj.items.clone().unwrap_or_else(|| Items { type_: Some("object".into()), ..Default::default() });
            let t = items.type_.clone();
            let f = items.format.map(str::to_string);
            let mut args = Args::new();
            if let Some(JsonNode::Array(an)) = items.extra.get("enum") {
                args.put(PropertyId::Enum, Arg::List(an.iter().map(|n| n.text_value().map(str::to_string).unwrap_or_else(|| "null".into())).collect()));
            }
            let inner = property_builder::build(t.as_deref(), f.as_deref(), &args)?;
            am.items = Some(match inner {
                Some(i) if i.kind != PropertyKind::Untyped => i.into_ref(),
                _ => match &items.ref_ {
                    Some(r) => Property::new_ref(r, Some(v2::RefFormat::Internal)).into_ref(),
                    None => Property::new_ref(t.as_deref().ok_or_else(null_ref)?, Some(v2::RefFormat::Internal)).into_ref(),
                },
            });
            return Ok(Some(am.into_ref()));
        }
        let mut args = Args::new();
        if let Some(e) = &obj.enum_values && !e.is_empty() {
            args.put(PropertyId::Enum, Arg::List(e.clone()));
        }
        if let Some(m) = &obj.minimum {
            args.put(PropertyId::Minimum, Arg::Decimal(BigDecimal::parse(m).map_err(|_| property_builder::number_format_bigdecimal(m))?));
        }
        if let Some(m) = &obj.maximum {
            args.put(PropertyId::Maximum, Arg::Decimal(BigDecimal::parse(m).map_err(|_| property_builder::number_format_bigdecimal(m))?));
        }
        let built = property_builder::build(ty.as_deref(), format.as_deref(), &args)?;
        Ok(match built {
            Some(i) if i.kind != PropertyKind::Untyped => Some(i.into_ref()),
            _ => {
                if let Some(r) = &obj.ref_ {
                    Some(Property::new_ref(r, Some(v2::RefFormat::Internal)).into_ref())
                } else {
                    ty.as_deref().filter(|t| *t != "void").map(|t| Property::new_ref(t, Some(v2::RefFormat::Internal)).into_ref())
                }
            }
        })
    }

    fn convert_operation(&self, tag: Option<&str>, op: &Operation12, decl: &ApiDeclaration) -> JResult<(Option<&'static str>, v2::OperationRef)> {
        let mut method = op.method;
        if method.is_none() {
            let node = op.typed.extra.get("httpMethod").ok_or_else(|| npe_msg("Cannot invoke \"com.fasterxml.jackson.databind.JsonNode.asText()\" because \"node\" is null"))?;
            method = enum_value(node, &METHODS).map_err(|_| npe())?;
        }
        let mut out = v2::Operation::new();
        out.summary = op.summary.clone();
        out.description = op.notes.clone();
        out.operation_id = op.nickname.clone();
        if let Some(t) = tag {
            out.tags.get_or_insert_with(Vec::new).push(t.to_string());
        }
        for p in op.parameters.clone().ok_or_else(npe)? {
            let p = p.ok_or_else(npe)?;
            out.parameters.get_or_insert_with(Vec::new).push(self.convert_parameter(&p)?);
        }
        let consumes = op.consumes.clone().filter(|c| !c.is_empty()).or_else(|| decl.consumes.clone());
        if let Some(c) = consumes {
            for x in c {
                out.consumes.get_or_insert_with(Vec::new).push(x);
            }
        }
        let produces = op.produces.clone().filter(|c| !c.is_empty()).or_else(|| decl.produces.clone());
        if let Some(c) = produces {
            for x in c {
                out.produces.get_or_insert_with(Vec::new).push(x);
            }
        }
        for m in op.response_messages.clone().ok_or_else(npe)? {
            let m = m.ok_or_else(npe)?;
            let mut response = v2::Response::new();
            response.description = m.message.clone();
            if let Some(rm) = &m.response_model {
                response.schema = Some(V2Model::new_ref(&format!("#/definitions/{rm}"), None).into_ref());
            }
            let code = m.code.map(|c| c.to_string()).unwrap_or_else(|| "null".into());
            out.responses.get_or_insert_with(v2::Responses::new).insert(code, Some(response.into_ref()));
        }
        let response_property = self.model_from_typed(&op.typed)?;
        let mut response = v2::Response::new();
        response.description = Some("success".into());
        response.schema = response_property.clone();
        if out.responses.is_none() {
            out.responses.get_or_insert_with(v2::Responses::new).insert("default".into(), Some(response.into_ref()));
        } else if response_property.is_some() {
            out.responses.get_or_insert_with(v2::Responses::new).insert("200".into(), Some(response.into_ref()));
        }
        for (name, scopes) in op.authorizations.clone().ok_or_else(npe)? {
            let scopes = scopes.ok_or_else(npe)?;
            let mut updated = Vec::new();
            for sc in scopes {
                let sc = sc.ok_or_else(npe)?;
                updated.push(sc.scope.unwrap_or_else(|| "null".into()));
            }
            let mut req = IndexMap::new();
            req.insert(name, updated);
            out.security.get_or_insert_with(Vec::new).push(req);
        }
        Ok((method, out.into_ref()))
    }

    /// `convert(resourceListing, apiDeclarations)`
    fn convert(&self, listing: &ResourceListing, decls: &[ApiDeclaration]) -> JResult<v2::Swagger> {
        let mut info = v2::Info::default();
        if let Some(api_info) = &listing.info {
            info.description = api_info.description.clone();
            info.version = listing.api_version.clone();
            info.title = api_info.title.clone();
            info.terms_of_service = api_info.terms_of_service_url.clone();
            info.contact = api_info.contact.as_ref().map(|c| v2::Contact { url: Some(c.clone()), ..Default::default() });
            info.license = api_info.license.as_ref().map(|l| v2::License { name: Some(l.clone()), url: api_info.license_url.clone(), ..Default::default() });
        } else if listing.api_version.is_some() {
            info.version = listing.api_version.clone();
        }
        // `new HashMap()` for paths and definitions (their iteration order is kept when serialized)
        let paths: JMap<usize> = JMap::with_kind(MapKind::Hash);
        let mut path_objs: Vec<v2::PathRef> = Vec::new();
        let definitions: JMap<usize> = JMap::with_kind(MapKind::Hash);
        let mut model_objs: Vec<v2::ModelRef> = Vec::new();
        let mut base_path: Option<String> = None;
        for decl in decls {
            let tag = decl.resource_path.clone().map(|t| t.replace('/', ""));
            if base_path.is_none() {
                base_path = decl.base_path.clone();
            }
            for api in decl.apis.clone().ok_or_else(npe)? {
                let api = api.ok_or_else(npe)?;
                let api_path = api.path.clone().ok_or_else(null_key)?;
                let path = match paths.get(&api_path) {
                    Some(i) => path_objs[i].clone(),
                    None => {
                        let p = v2::Path::new().into_ref();
                        paths.put(api_path.clone(), path_objs.len());
                        path_objs.push(p.clone());
                        p
                    }
                };
                for op in api.operations.clone().ok_or_else(npe)? {
                    let op = op.ok_or_else(npe)?;
                    let (method, operation) = self.convert_operation(tag.as_deref(), &op, decl)?;
                    if let Some(m) = method {
                        let mut p = path.borrow_mut();
                        match m {
                            "get" => p.get = Some(operation),
                            "put" => p.put = Some(operation),
                            "head" => p.head = Some(operation),
                            "post" => p.post = Some(operation),
                            "delete" => p.delete = Some(operation),
                            "patch" => p.patch = Some(operation),
                            _ => p.options = Some(operation),
                        }
                    }
                }
            }
            for (key, model) in decl.models.clone().ok_or_else(npe)? {
                let model = model.ok_or_else(npe)?;
                let converted = self.convert_model(&model)?;
                match definitions.get(&key) {
                    Some(i) => model_objs[i] = converted,
                    None => {
                        definitions.put(key.clone(), model_objs.len());
                        model_objs.push(converted);
                    }
                }
            }
        }
        let mut host = None;
        let mut scheme = "http".to_string();
        if let Some(bp) = base_path.clone() {
            let parts = crate::parser::ref_utils::java_split(&bp, "://");
            let mut bp = bp;
            if parts.len() == 2 {
                scheme = parts[0].clone();
                match parts[1].find('/') {
                    Some(pos) => {
                        host = Some(parts[1][..pos].to_string());
                        bp = parts[1][pos..].to_string();
                    }
                    None => {
                        host = Some(parts[1].clone());
                        bp = "/".into();
                    }
                }
            }
            if !bp.starts_with('/') {
                bp = format!("/{bp}");
            }
            base_path = Some(bp);
        }
        let mut swagger = v2::Swagger {
            host,
            schemes: Some(vec![v2::Scheme::for_value(&scheme)]),
            base_path,
            info: Some(info),
            paths: Some(paths.entries().into_iter().map(|(k, i)| (k, path_objs[i].clone())).collect()),
            definitions: Some(definitions.entries().into_iter().map(|(k, i)| (k, model_objs[i].clone())).collect()),
            ..v2::Swagger::default()
        };
        if let Some(auths) = &listing.authorizations {
            for (nickname, auth) in auths {
                let Some(auth) = auth else { return Err(npe()) };
                match auth {
                    Authorization::OAuth2 { scopes, grant_types } => {
                        let Some(grant_types) = grant_types else { return Err(npe()) };
                        let flow = if let Some(implicit) = &grant_types.implicit {
                            let url = implicit.clone().ok_or_else(npe)?;
                            let mut def = v2::SecuritySchemeDefinition::new(v2::SecuritySchemeKind::OAuth2);
                            def.authorization_url = url;
                            def.flow = Some("implicit".into());
                            Some(def)
                        } else if let Some((req, tok)) = &grant_types.authorization_code {
                            let req = req.clone().ok_or_else(npe)?;
                            let tok = tok.clone().ok_or_else(npe)?;
                            let mut def = v2::SecuritySchemeDefinition::new(v2::SecuritySchemeKind::OAuth2);
                            def.token_url = tok;
                            def.authorization_url = req;
                            def.flow = Some("accessCode".into());
                            Some(def)
                        } else {
                            None
                        };
                        if let Some(mut def) = flow {
                            for sc in scopes.clone().ok_or_else(npe)? {
                                let sc = sc.ok_or_else(npe)?;
                                let name = sc.scope.clone().ok_or_else(null_key)?;
                                def.scopes.get_or_insert_with(IndexMap::new).insert(name, sc.description);
                            }
                            let defs = swagger.security_definitions.get_or_insert_with(IndexMap::new);
                            if !defs.contains_key(nickname) {
                                defs.insert(nickname.clone(), def);
                            }
                        }
                    }
                    Authorization::ApiKey { pass_as, keyname } => {
                        let mut def = v2::SecuritySchemeDefinition::new(v2::SecuritySchemeKind::ApiKey);
                        def.in_ = Some(if *pass_as == Some("header") { v2::In::Header } else { v2::In::Query });
                        def.name = keyname.clone();
                        swagger.security_definitions.get_or_insert_with(IndexMap::new).insert(nickname.clone(), def);
                    }
                    Authorization::Basic => {
                        swagger.security_definitions.get_or_insert_with(IndexMap::new).insert(nickname.clone(), v2::SecuritySchemeDefinition::new(v2::SecuritySchemeKind::Basic));
                    }
                }
            }
        }
        Ok(swagger)
    }
}

/// `PropertyBuilder.toModel(property)`
fn to_model(p: &Property) -> Option<v2::ModelRef> {
    let create = |p: &Property| {
        let mut m = V2Model::new(ModelKind::Impl);
        m.type_ = p.type_.clone();
        m.format = p.format.clone();
        m.set_description(p.description.clone());
        m
    };
    let m = match p.kind {
        PropertyKind::ByteArray | PropertyKind::Email | PropertyKind::String | PropertyKind::Uuid => {
            let mut m = create(p);
            m.default_value = p.default_string.clone();
            m
        }
        PropertyKind::Integer | PropertyKind::Long | PropertyKind::Float | PropertyKind::Double => {
            let mut m = create(p);
            if let Some(d) = &p.default_value {
                m.default_value = Some(d.to_java_string());
            }
            m
        }
        PropertyKind::Ref => {
            let mut m = V2Model::new_ref(&p.original_ref().unwrap_or_default(), p.ref_format());
            m.set_description(p.description.clone());
            m
        }
        PropertyKind::Array => {
            let mut m = V2Model::new(ModelKind::Array);
            m.items = p.items.clone();
            m.set_description(p.description.clone());
            m
        }
        PropertyKind::Map => {
            let mut m = create(p);
            m.set_additional_properties(p.additional_properties.clone());
            m
        }
        // no Processor matches these classes
        PropertyKind::Password | PropertyKind::Composed | PropertyKind::BooleanValue => return None,
        _ => create(p),
    };
    Some(m.into_ref())
}
