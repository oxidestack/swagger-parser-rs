//! swagger-parser-v2-converter 2.1.48: `SwaggerConverter` (Swagger 2.0 → OpenAPI 3.0) and
//! `SwaggerInventory`.

use crate::core::{AuthorizationValue, ParseOptions, SwaggerParseResult, SwaggerParserExtension};
use crate::java::{JList, JMap, JResult, JavaException, MapKind};
use crate::models::deserialize::{bean_schema, schema_from_tree_nullable};
use crate::models::*;
use crate::v2::json::{model_to_tree_with, property_to_tree_with};
use crate::v2::models as v2;
use crate::v2::models::{ModelKind, ModelRef, ParameterKind as V2ParameterKind, ParameterRef, PropertyKind, PropertyRef};
use crate::v2::parser::SwaggerParser;
use crate::v2::property_builder::model_to_property;
use crate::v2::resolver::SwaggerResolver;
use jackson::{BigDecimal, JacksonError, JavaObject};
use std::cell::RefCell;

const STRIPPED_EXTENSION_KEYS: [&str; 3] = ["x-example", "x-examples", "x-nullable"];

fn jackson_err(e: JacksonError) -> JavaException {
    JavaException::new("java.lang.IllegalArgumentException", e.message)
}

fn npe() -> JavaException {
    JavaException::without_message("java.lang.NullPointerException")
}

/// The per-instance state of a `SwaggerConverter` (its fields in Java).
#[derive(Default)]
struct State {
    global_consumes: Vec<String>,
    global_produces: Vec<String>,
    components: Option<Components>,
    global_v2_parameters: std::collections::HashMap<String, ParameterRef>,
}

/// `io.swagger.v3.parser.converter.SwaggerConverter`. Each read uses fresh converter state (Java keeps the
/// global consumes/produces, components and v2 parameters on the instance, so a reused Java instance
/// carries them over between reads).
#[derive(Clone, Copy, Debug, Default)]
pub struct SwaggerConverter;

impl SwaggerConverter {
    /// `new SwaggerConverter()`.
    pub fn new() -> SwaggerConverter {
        SwaggerConverter
    }

    /// `readLocation(url, auths, options)`; `Err` where Java throws out of the method.
    pub fn try_read_location(&self, url: &str, auths: Option<&[AuthorizationValue]>, options: Option<&ParseOptions>) -> JResult<Option<SwaggerParseResult>> {
        Conversion::default().try_read_location(url, auths, options)
    }

    /// `readContents(swaggerAsString, auth, options)`; `Err` where Java throws out of the method.
    pub fn try_read_contents(&self, s: &str, auths: Option<&[AuthorizationValue]>, options: Option<&ParseOptions>) -> JResult<Option<SwaggerParseResult>> {
        Conversion::default().try_read_contents(s, auths, options)
    }

    /// `convert(SwaggerDeserializationResult)`
    pub fn convert(&self, parse: Option<v2::SwaggerDeserializationResult>) -> JResult<Option<SwaggerParseResult>> {
        Conversion::default().convert_result(parse)
    }

    /// `convert(List<AuthorizationValue>)`
    pub fn convert_auths(&self, auths: Option<&[AuthorizationValue]>) -> Vec<v2::AuthorizationValue> {
        convert_auths(auths)
    }
}

/// `convert(List<AuthorizationValue>)`: copies type, value and key name into new swagger-models
/// `AuthorizationValue`s. Their url matcher is left null, which the v1 `RemoteUrl` treats as matching
/// every URL — so the converted values match every URL whatever the originals' matchers.
fn convert_auths(auths: Option<&[AuthorizationValue]>) -> Vec<v2::AuthorizationValue> {
    auths
        .unwrap_or(&[])
        .iter()
        .map(|a| v2::AuthorizationValue { type_: a.type_.clone(), value: a.value.clone(), key_name: a.key_name.clone(), ..Default::default() })
        .collect()
}

/// One conversion (the state a Java `SwaggerConverter` instance holds).
#[derive(Default)]
struct Conversion {
    state: RefCell<State>,
}

impl Conversion {
    fn components(&self) -> Components {
        self.state.borrow_mut().components.get_or_insert_with(Components::new).clone()
    }

    /// `readLocation(url, auths, options)`; `Err` where Java throws out of the method.
    pub fn try_read_location(&self, url: &str, auths: Option<&[AuthorizationValue]>, options: Option<&ParseOptions>) -> JResult<Option<SwaggerParseResult>> {
        let resolve = options.is_some_and(|o| o.resolve);
        let result = SwaggerParser::new().read_with_info(Some(url), &convert_auths(auths), resolve);
        self.read_result(result, auths, options)
    }

    /// `readContents(swaggerAsString, auth, options)`; `Err` where Java throws out of the method.
    pub fn try_read_contents(&self, swagger_as_string: &str, auths: Option<&[AuthorizationValue]>, options: Option<&ParseOptions>) -> JResult<Option<SwaggerParseResult>> {
        let mut result = SwaggerParser::new().read_with_info_contents(Some(swagger_as_string), options.is_none_or(|o| o.resolve));
        if options.is_some_and(|o| o.resolve) {
            result.swagger = SwaggerResolver::new(result.swagger.clone(), convert_auths(auths), None).resolve()?;
        }
        self.read_result(Some(result), auths, options)
    }

    fn read_result(&self, result: Option<v2::SwaggerDeserializationResult>, auths: Option<&[AuthorizationValue]>, options: Option<&ParseOptions>) -> JResult<Option<SwaggerParseResult>> {
        let Some(mut out) = self.convert_result(result)? else { return Ok(None) };
        if let (Some(openapi), Some(options)) = (out.openapi.clone(), options) {
            if options.resolve_fully {
                crate::parser::resolver_fully::ResolverFully::new(options).resolve_fully(Some(&openapi))?;
            }
            if options.flatten {
                // `try { … } catch (Exception ignore) {}`
                if let Some(yaml) = yaml_pretty(&openapi) {
                    let v3 = crate::parser::v3_parser::OpenAPIV3Parser::new().read_contents(&yaml, auths, Some(options));
                    out.openapi = v3.openapi.clone();
                    match &mut out.messages {
                        Some(m) => {
                            m.extend(v3.messages.clone().unwrap_or_default());
                            let mut seen = std::collections::HashSet::new();
                            m.retain(|x| seen.insert(x.clone()));
                        }
                        None => out.messages = v3.messages.clone(),
                    }
                }
            }
        }
        Ok(Some(out))
    }

    /// `convert(SwaggerDeserializationResult)`
    pub fn convert_result(&self, parse: Option<v2::SwaggerDeserializationResult>) -> JResult<Option<SwaggerParseResult>> {
        let Some(parse) = parse else { return Ok(None) };
        let mut output = SwaggerParseResult { messages: parse.messages.clone(), ..Default::default() };
        let Some(swagger) = parse.swagger.clone() else { return Ok(Some(output)) };
        let openapi = OpenAPI::new();
        let inventory = SwaggerInventory::new().process(&swagger)?;
        let sw = swagger.borrow().clone();

        if let Some(ext) = &sw.vendor_extensions {
            openapi.set_extensions(convert_extensions(Some(ext)));
        }
        openapi.add_extension("x-original-swagger-version", sw.swagger.clone().map(JavaObject::string).unwrap_or(JavaObject::Null));
        if let Some(d) = &sw.external_docs {
            openapi.set_external_docs(convert_external_docs(d));
        }
        if let Some(info) = &sw.info {
            openapi.set_info(convert_info(info));
        }
        openapi.set_servers(convert_servers(sw.schemes.as_deref(), sw.host.as_deref(), sw.base_path.as_deref())?);
        if let Some(tags) = &sw.tags {
            openapi.set_tags(convert_tags(tags));
        }
        if let Some(c) = &sw.consumes {
            self.state.borrow_mut().global_consumes.extend(c.iter().cloned());
        }
        if let Some(p) = &sw.produces {
            self.state.borrow_mut().global_produces.extend(p.iter().cloned());
        }
        if let Some(sec) = &sw.security && !sec.is_empty() {
            let list = JList::new();
            for r in sec {
                let s = SecurityRequirement::new();
                for (k, v) in &r.requirements {
                    s.add_list_items(k, JList::from_vec(v.clone()));
                }
                list.add(s);
            }
            openapi.set_security(list);
        }

        for model in inventory.models.iter() {
            let is_ref = model.borrow().kind == ModelKind::Ref;
            if is_ref {
                let r = model.borrow().get_ref().unwrap_or_default();
                if let Some(rest) = r.strip_prefix("#/definitions") {
                    model.borrow_mut().set_ref(&format!("#/components/schemas{rest}"));
                }
            }
        }
        for property in inventory.properties.iter().flatten() {
            let kind = property.borrow().kind;
            if kind == PropertyKind::Ref {
                rewrite_definition_ref(property)?;
            }
            if kind == PropertyKind::Composed {
                let all_of = property.borrow().all_of.clone();
                if let Some(all_of) = all_of {
                    for item in all_of {
                        let Some(item) = item else { continue };
                        if item.borrow().kind == PropertyKind::Ref {
                            rewrite_definition_ref(&item)?;
                        }
                    }
                }
            }
        }

        if let Some(params) = &sw.parameters {
            {
                let mut st = self.state.borrow_mut();
                for (k, v) in params {
                    st.global_v2_parameters.insert(k.clone(), v.clone());
                }
            }
            for (k, v) in params {
                let in_ = v.borrow().in_.clone();
                let components = self.components();
                match in_.as_deref() {
                    Some("body") => {
                        let rb = self.convert_parameter_to_request_body(v, None)?;
                        components.get_request_bodies_or_create().put(k.clone(), rb);
                    }
                    Some("formData") => {
                        let s = convert_serializable(&v.borrow())?;
                        components.get_schemas_or_create().put(format!("formData_{k}"), s);
                    }
                    _ => {
                        let p = self.convert_parameter(v)?;
                        components.get_parameters_or_create().put(k.clone(), p);
                    }
                }
            }
        }

        let v3_paths = Paths::new();
        if let Some(paths) = &sw.paths {
            for (name, path) in paths {
                v3_paths.put(name.clone(), self.convert_path(path)?);
            }
        }
        openapi.set_paths(v3_paths);

        if let Some(responses) = &sw.responses {
            for (k, v) in responses {
                let r = match v {
                    Some(v) => self.convert_response(v, None)?,
                    None => return Err(npe()),
                };
                self.components().get_responses_or_create().put(k.clone(), r);
            }
        }
        if let Some(defs) = &sw.definitions {
            for (k, m) in defs {
                let s = self.convert_model(Some(m))?;
                let components = self.components();
                let map = components.get_schemas_or_create();
                match s {
                    Some(s) => {
                        map.put(k.clone(), s);
                    }
                    None => {
                        map.put_null_value(k);
                    }
                }
            }
        }
        if let Some(defs) = &sw.security_definitions {
            for (k, v) in defs {
                let s = convert_security_scheme(v);
                self.components().get_security_schemes_or_create().put(k.clone(), s);
            }
        }
        openapi.set_components(self.components());
        output.openapi = Some(openapi);
        Ok(Some(output))
    }

    fn is_ref_a_body_param(&self, p: &ParameterRef) -> JResult<bool> {
        if p.borrow().kind != V2ParameterKind::Ref {
            return Ok(false);
        }
        let simple = p.borrow().simple_ref().unwrap_or_default();
        let st = self.state.borrow_mut();
        match st.global_v2_parameters.get(&simple) {
            Some(x) => Ok(x.borrow().in_.as_deref() == Some("body")),
            None => Err(npe()),
        }
    }

    fn is_ref_a_form_param(&self, p: &ParameterRef) -> JResult<bool> {
        if p.borrow().kind != V2ParameterKind::Ref {
            return Ok(false);
        }
        let simple = p.borrow().simple_ref().unwrap_or_default();
        let st = self.state.borrow_mut();
        match st.global_v2_parameters.get(&simple) {
            Some(x) => Ok(x.borrow().in_.as_deref() == Some("formData")),
            None => Err(npe()),
        }
    }

    /// `convert(Path)`
    pub fn convert_path(&self, v2_path: &v2::PathRef) -> JResult<PathItem> {
        let v3 = PathItem::new();
        let p = v2_path.borrow().clone();
        if p.is_ref {
            v3.set_ref(p.get_ref());
            return Ok(v3);
        }
        if let Some(params) = &p.parameters {
            for param in params {
                v3.add_parameters_item(self.convert_parameter(param)?);
            }
        }
        if let Some(op) = &p.get {
            v3.set_get(self.convert_operation(op)?);
        }
        if let Some(op) = &p.put {
            v3.set_put(self.convert_operation(op)?);
        }
        if let Some(op) = &p.post {
            v3.set_post(self.convert_operation(op)?);
        }
        if let Some(op) = &p.patch {
            v3.set_patch(self.convert_operation(op)?);
        }
        if let Some(op) = &p.delete {
            v3.set_delete(self.convert_operation(op)?);
        }
        if let Some(op) = &p.head {
            v3.set_head(self.convert_operation(op)?);
        }
        if let Some(op) = &p.options {
            v3.set_options(self.convert_operation(op)?);
        }
        v3.set_extensions(convert_extensions(Some(&p.vendor_extensions)));
        Ok(v3)
    }

    /// `convert(io.swagger.models.Operation)`
    pub fn convert_operation(&self, v2_op: &v2::OperationRef) -> JResult<Operation> {
        let op = Operation::new();
        let o = v2_op.borrow().clone();
        if o.description.as_deref().is_some_and(is_not_blank) {
            op.set_description(o.description.clone());
        }
        if o.summary.as_deref().is_some_and(is_not_blank) {
            op.set_summary(o.summary.clone());
        }
        op.set_deprecated(o.deprecated);
        op.set_operation_id(o.operation_id.clone());
        op.set_extensions(convert_extensions(Some(&o.vendor_extensions)));
        op.set_tags(o.tags.clone().map(JList::from_vec));

        if let Some(params) = &o.parameters {
            let mut form_params: Vec<ParameterRef> = Vec::new();
            for param in params {
                let in_ = param.borrow().in_.clone();
                match in_.as_deref() {
                    Some("formData") => form_params.push(param.clone()),
                    Some("body") => {
                        op.set_request_body(self.convert_parameter_to_request_body(param, o.consumes.as_deref())?);
                        op.add_extension("x-codegen-request-body-name", param.borrow().name.clone().map(JavaObject::string).unwrap_or(JavaObject::Null));
                    }
                    _ => {
                        let convert = self.convert_parameter(param)?;
                        let r = convert.get_ref();
                        if r.as_deref().is_some_and(|r| r.starts_with("#/components/requestBodies/")) && self.is_ref_a_body_param(param)? {
                            let rb = RequestBody::new();
                            rb.set_ref(r);
                            op.set_request_body(rb);
                        } else if r.as_deref().is_some_and(|r| r.starts_with("#/components/schemas/")) && self.is_ref_a_form_param(param)? {
                            form_params.push(param.clone());
                        } else {
                            op.add_parameters_item(convert);
                        }
                    }
                }
            }
            if !form_params.is_empty() {
                let body = self.convert_form_data_to_request_body(&form_params, o.consumes.as_deref())?;
                if let Some(content) = body.get_content() {
                    for (_, mt) in content.entries() {
                        if let Some(schema) = mt.get_schema() && schema.get_required().is_some_and(|r| r.size() > 0) {
                            body.set_required(true);
                        }
                    }
                }
                op.set_request_body(body);
            }
        }

        if let Some(responses) = &o.responses {
            for (code, r) in responses {
                let response = match r {
                    Some(r) => self.convert_response(r, o.produces.as_deref())?,
                    None => return Err(npe()),
                };
                let responses = op.get_responses().unwrap_or_default();
                op.set_responses(responses.add_api_response(code, response));
            }
        }
        if let Some(d) = &o.external_docs {
            op.set_external_docs(convert_external_docs(d));
        }
        if let Some(sec) = &o.security {
            let list = JList::new();
            for m in sec {
                let s = SecurityRequirement::new();
                for (k, v) in m {
                    s.add_list_items(k, JList::from_vec(v.clone()));
                }
                list.add(s);
            }
            op.set_security(list);
        }
        Ok(op)
    }

    fn convert_form_data_to_request_body(&self, form_params: &[ParameterRef], consumes: Option<&[String]>) -> JResult<RequestBody> {
        let body = RequestBody::new();
        let form_schema = Schema::object_schema();
        for param in form_params {
            let (sp, schema, name) = if param.borrow().kind == V2ParameterKind::Ref {
                let simple = param.borrow().simple_ref().unwrap_or_default();
                let sp = self.state.borrow_mut().global_v2_parameters.get(&simple).cloned().ok_or_else(npe)?;
                let name = self.components().get_schemas().and_then(|m| m.get(&format!("formData_{simple}"))).ok_or_else(npe)?.get_name();
                let schema = Schema::new();
                schema.set_ref(format!("#/components/schemas/formData_{simple}"));
                (sp, schema, name)
            } else {
                let schema = convert_serializable(&param.borrow())?;
                let name = schema.get_name();
                (param.clone(), schema, name)
            };
            let spb = sp.borrow();
            if !spb.kind.is_serializable() {
                return Err(JavaException::class_cast("io.swagger.models.parameters.BodyParameter", "io.swagger.models.parameters.SerializableParameter"));
            }
            if spb.required {
                form_schema.add_required_item(spb.name.as_deref().unwrap_or("null"));
            }
            add_property_nullable_key(&form_schema, name.as_deref(), schema);
        }
        let mut media_types: Vec<String> = self.state.borrow_mut().global_consumes.clone();
        if let Some(c) = consumes && !c.is_empty() {
            media_types = c.to_vec();
        }
        if media_types.is_empty() {
            media_types.push("multipart/form-data".into());
        }
        let content = Content::new();
        for t in media_types {
            let mt = MediaType::new();
            mt.set_schema(form_schema.clone());
            content.add_media_type(&t, mt);
        }
        body.set_content(content);
        Ok(body)
    }

    fn convert_parameter_to_request_body(&self, param: &ParameterRef, consumes: Option<&[String]>) -> JResult<RequestBody> {
        let body = RequestBody::new();
        let bp = param.borrow().clone();
        if bp.kind != V2ParameterKind::Body {
            return Err(JavaException::class_cast(&format!("io.swagger.models.parameters.{}", v2_parameter_class(bp.kind)), "io.swagger.models.parameters.BodyParameter"));
        }
        let mut media_types: Vec<String> = self.state.borrow_mut().global_consumes.clone();
        if let Some(c) = consumes && !c.is_empty() {
            media_types = c.to_vec();
        }
        if media_types.is_empty() {
            media_types.push("*/*".into());
        }
        if bp.description.as_deref().is_some_and(is_not_blank) {
            body.set_description(bp.description.clone());
        }
        body.set_required(bp.required);
        let content = Content::new();
        for t in media_types {
            let mt = MediaType::new();
            mt.set_schema(self.convert_model(bp.schema.as_ref())?);
            content.add_media_type(&t, mt);
            if bp.description.as_deref().is_some_and(is_not_blank) {
                body.set_description(bp.description.clone());
            }
        }
        convert_examples(bp.examples.as_ref().map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect::<Vec<_>>()), &content);
        body.set_extensions(convert_extensions(Some(&bp.vendor_extensions)));
        body.set_content(content);
        Ok(body)
    }

    /// `convert(Response, produces)`
    pub fn convert_response(&self, v2r: &v2::ResponseRef, produces: Option<&[String]>) -> JResult<ApiResponse> {
        let response = ApiResponse::new();
        let content = Content::new();
        if v2r.borrow().is_ref {
            let r = v2r.borrow().get_ref().ok_or_else(npe)?;
            if let Some(rest) = r.strip_prefix("#/responses") {
                v2r.borrow_mut().set_ref(&format!("#/components/responses{rest}"));
            }
            response.set_ref(v2r.borrow().get_ref());
            return Ok(response);
        }
        let r = v2r.borrow().clone();
        let mut media_types: Vec<String> = self.state.borrow_mut().global_produces.clone();
        if let Some(p) = produces {
            media_types = p.to_vec();
        }
        if media_types.is_empty() {
            media_types.push("*/*".into());
        }
        response.set_description(r.description.clone());
        if r.schema.is_some() {
            let schema = self.convert_model(r.schema.as_ref())?.ok_or_else(npe)?;
            let schema = convert_file_schema(schema);
            for t in &media_types {
                let mt = MediaType::new();
                mt.set_schema(schema.clone());
                content.add_media_type(t, mt);
            }
            response.set_content(content.clone());
        }
        response.set_content(convert_examples(r.examples.as_ref().map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect()), &content));
        response.set_extensions(convert_extensions(Some(&r.vendor_extensions)));
        if let Some(headers) = &r.headers && !headers.is_empty() {
            let result: JMap<Header> = JMap::with_kind(MapKind::Hash);
            for (k, v) in headers {
                let h = convert_header(v.as_ref())?;
                result.put(k.clone(), h);
            }
            response.set_headers(result);
        }
        Ok(response)
    }

    /// `convert(io.swagger.models.parameters.Parameter)`
    pub fn convert_parameter(&self, v2p: &ParameterRef) -> JResult<Parameter> {
        let v3 = Parameter::new();
        let p = v2p.borrow().clone();
        if p.description.as_deref().is_some_and(is_not_blank) {
            v3.set_description(p.description.clone());
        }
        if p.kind.is_serializable() {
            v3.set_allow_empty_value(p.allow_empty_value);
        }
        v3.set_in(p.in_.clone());
        v3.set_name(p.name.clone());
        if let Some(ex) = p.vendor_extensions.get("x-example") && !ex.is_null() {
            v3.set_example(ex);
        }
        let mut schema: Option<Schema> = None;
        if p.kind == V2ParameterKind::Ref {
            let r = p.get_ref().ok_or_else(npe)?;
            if let Some(rest) = r.strip_prefix("#/parameters") {
                let simple = p.simple_ref().unwrap_or_default();
                let components = self.components();
                let mut updated = "#/components/".to_string();
                if components.get_request_bodies().is_some_and(|m| m.get(&simple).is_some()) {
                    updated.push_str("requestBodies");
                } else if components.get_schemas().is_some_and(|m| m.get(&format!("formData_{simple}")).is_some()) {
                    updated.push_str("schemas");
                } else {
                    updated.push_str("parameters");
                }
                updated.push_str(rest);
                v2p.borrow_mut().set_ref(&updated);
            }
            v3.set_ref(v2p.borrow().get_ref());
        } else if p.kind.is_serializable() {
            let s = if p.type_.as_deref() == Some("array") {
                let a = Schema::array_schema();
                let cf = match p.collection_format.as_deref() {
                    None | Some("") => "csv".to_string(),
                    Some(c) => c.to_string(),
                };
                let in_ = p.in_.as_deref();
                match cf.as_str() {
                    "ssv" => {
                        if in_ == Some("query") {
                            v3.set_style(ParameterStyle::SpaceDelimited);
                        }
                    }
                    "pipes" => {
                        if in_ == Some("query") {
                            v3.set_style(ParameterStyle::PipeDelimited);
                        }
                    }
                    "tsv" => {}
                    "multi" => {
                        if in_ == Some("query") {
                            v3.set_style(ParameterStyle::Form);
                            v3.set_explode(true);
                        }
                    }
                    _ => {
                        if in_ == Some("query") {
                            v3.set_style(ParameterStyle::Form);
                            v3.set_explode(false);
                        } else if in_ == Some("header") || in_ == Some("path") {
                            v3.set_style(ParameterStyle::Simple);
                            v3.set_explode(false);
                        }
                    }
                }
                a.set_items(convert_property(p.items.as_ref())?);
                if p.max_items.is_some() {
                    a.set_max_items(p.max_items);
                }
                if p.min_items.is_some() {
                    a.set_min_items(p.min_items);
                }
                if p.unique_items.is_some() {
                    a.set_unique_items(p.unique_items);
                }
                a
            } else {
                let s = crate::parser::deserializer::create_schema(p.type_.as_deref().unwrap_or(""), p.format.as_deref());
                s.set_type(p.type_.clone());
                s.set_format(p.format.clone());
                s.set_maximum(p.maximum.clone());
                s.set_exclusive_maximum(p.exclusive_maximum);
                s.set_minimum(p.minimum.clone());
                s.set_exclusive_minimum(p.exclusive_minimum);
                s.set_min_length(p.min_length);
                s.set_max_length(p.max_length);
                if let Some(m) = &p.multiple_of {
                    s.set_multiple_of(parse_java_big_decimal(&m.to_java_string())?);
                }
                s.set_pattern(p.pattern.clone());
                s
            };
            if let Some(e) = &p.enum_ {
                for item in e {
                    let t = p.type_.as_deref().unwrap_or("object");
                    let v = match t {
                        "integer" => JavaObject::Integer(v2::java_parse_int(item).ok_or_else(|| number_format_for_input(item))?),
                        "number" => JavaObject::BigDecimal(parse_java_big_decimal(item)?.into()),
                        "boolean" => JavaObject::Boolean(item.eq_ignore_ascii_case("true")),
                        _ => JavaObject::string(item.as_str()),
                    };
                    s.add_enum_item_object(v);
                }
            }
            if let Some(n) = p.vendor_extensions.get("x-nullable") && !n.is_null() {
                s.set_nullable(cast_boolean(&n)?);
            }
            s.set_extensions(convert_extensions(Some(&p.vendor_extensions)));
            if let Some(d) = p.get_default() {
                s.set_default(d);
            }
            schema = Some(s);
        }
        if p.required {
            v3.set_required(true);
        }
        v3.set_schema(schema);
        v3.set_extensions(convert_extensions(Some(&p.vendor_extensions)));
        Ok(v3)
    }

    /// `convert(io.swagger.models.Model)`; `Ok(None)` for a null model.
    pub fn convert_model(&self, v2m: Option<&ModelRef>) -> JResult<Option<Schema>> {
        let Some(v2m) = v2m else { return Ok(None) };
        if let Some(b) = v2m.borrow().get_boolean_value() {
            let s = Schema::new();
            s.set_boolean_schema_value(b);
            return Ok(Some(s));
        }
        let kind = v2m.borrow().kind;
        let result: Schema = match kind {
            ModelKind::Array => {
                let node = model_to_tree_with(&v2m.borrow(), false).map_err(jackson_err)?;
                let a = bean_schema(SchemaKind::Array, &node, Mapper::Json30).map_err(jackson_err)?;
                let items = v2m.borrow().items.clone();
                a.set_items(convert_property(items.as_ref())?);
                a
            }
            ModelKind::Composed => {
                let m = v2m.borrow().clone();
                let composed = Schema::composed_schema();
                composed.set_description(m.get_description());
                if let Some(e) = m.get_example() {
                    composed.set_example(e);
                }
                if let Some(d) = &m.external_docs {
                    composed.set_external_docs(convert_external_docs(d));
                }
                composed.set_title(m.title.clone());
                composed.set_extensions(convert_extensions(m.get_vendor_extensions().as_ref()));
                let all_of = JList::new();
                for x in &m.all_of {
                    all_of.add(self.convert_model(Some(x))?.ok_or_else(npe)?);
                }
                composed.set_all_of(all_of);
                composed.set_required(m.get_required().map(JList::from_vec));
                self.add_properties(v2m, &composed)?;
                composed
            }
            _ => {
                let mut v2_discriminator = None;
                if kind == ModelKind::Impl {
                    let mut m = v2m.borrow_mut();
                    v2_discriminator = m.discriminator.take();
                }
                let is_file = kind == ModelKind::Impl && v2m.borrow().type_.as_deref() == Some("file");
                let node = model_to_tree_with(&v2m.borrow(), !is_file).map_err(jackson_err)?;
                // `convertValue(v2Model, Schema.class)` is null for an unknown `type`; Java carries on and
                // only fails where it dereferences the result
                let result: Option<Schema> = if is_file {
                    Some(bean_schema(SchemaKind::File, &node, Mapper::Json30).map_err(jackson_err)?)
                } else {
                    schema_from_tree_nullable(&node, Mapper::Json30).map_err(jackson_err)?
                };
                self.add_properties_nullable(v2m, result.as_ref())?;
                if kind == ModelKind::Impl {
                    let ap = v2m.borrow().additional_properties.clone();
                    if let Some(ap) = ap {
                        let r = result.as_ref().ok_or_else(npe)?;
                        let b = ap.borrow().boolean_value;
                        match b {
                            Some(b) => r.set_additional_properties(AdditionalProperties::Bool(b)),
                            None => r.set_additional_properties(convert_property(Some(&ap))?.map(AdditionalProperties::Schema)),
                        }
                    }
                } else if kind == ModelKind::Ref {
                    let r = v2m.borrow().get_ref().unwrap_or_default();
                    if let Some(rest) = r.strip_prefix("#/definitions") {
                        result.as_ref().ok_or_else(npe)?.set_ref(format!("#/components/schemas{rest}"));
                    }
                }
                if let Some(d) = v2_discriminator {
                    let disc = Discriminator::new();
                    disc.set_property_name(d);
                    result.as_ref().ok_or_else(npe)?.set_discriminator(disc);
                }
                let Some(result) = result else {
                    if let Some(ext) = v2m.borrow().get_vendor_extensions()
                        && ext.get("x-nullable").is_some_and(|n| !n.is_null())
                    {
                        return Err(npe());
                    }
                    return Ok(None);
                };
                result
            }
        };
        if let Some(ext) = v2m.borrow().get_vendor_extensions() && let Some(n) = ext.get("x-nullable") && !n.is_null() {
            result.set_nullable(cast_boolean(&n)?);
        }
        Ok(Some(result))
    }

    /// `addProperties(v2Model, schema)` where the schema may be null
    fn add_properties_nullable(&self, v2m: &ModelRef, schema: Option<&Schema>) -> JResult<()> {
        let has = v2m.borrow().properties.as_ref().is_some_and(|p| p.size() > 0);
        match schema {
            Some(s) => self.add_properties(v2m, s),
            None if has => Err(npe()),
            None => Ok(()),
        }
    }

    /// `addProperties(v2Model, schema)`
    fn add_properties(&self, v2m: &ModelRef, schema: &Schema) -> JResult<()> {
        let props = v2m.borrow().properties.clone();
        if let Some(props) = props && props.size() > 0 {
            for (k, v) in props.entries() {
                let s = convert_property(Some(&v))?;
                add_property_nullable(schema, &k, s);
            }
        }
        Ok(())
    }
}

impl SwaggerParserExtension for SwaggerConverter {
    fn read_location(&self, url: &str, auth: Option<&[AuthorizationValue]>, options: Option<&ParseOptions>) -> SwaggerParseResult {
        match SwaggerConverter::try_read_location(self, url, auth, options) {
            Ok(r) => r.unwrap_or_default(),
            Err(e) => SwaggerParseResult::of_error(e.get_message().unwrap_or("null")),
        }
    }

    fn read_contents(&self, s: &str, auth: Option<&[AuthorizationValue]>, options: Option<&ParseOptions>) -> SwaggerParseResult {
        match SwaggerConverter::try_read_contents(self, s, auth, options) {
            Ok(r) => r.unwrap_or_default(),
            Err(e) => SwaggerParseResult::of_error(e.get_message().unwrap_or("null")),
        }
    }

    fn try_read_location(&self, url: &str, auth: Option<&[AuthorizationValue]>, options: Option<&ParseOptions>) -> JResult<Option<SwaggerParseResult>> {
        SwaggerConverter::try_read_location(self, url, auth, options)
    }

    fn try_read_contents(&self, s: &str, auth: Option<&[AuthorizationValue]>, options: Option<&ParseOptions>) -> JResult<Option<SwaggerParseResult>> {
        SwaggerConverter::try_read_contents(self, s, auth, options)
    }
}

fn v2_parameter_class(k: V2ParameterKind) -> &'static str {
    match k {
        V2ParameterKind::Query => "QueryParameter",
        V2ParameterKind::Header => "HeaderParameter",
        V2ParameterKind::Path => "PathParameter",
        V2ParameterKind::Form => "FormParameter",
        V2ParameterKind::Cookie => "CookieParameter",
        V2ParameterKind::Body => "BodyParameter",
        V2ParameterKind::Ref => "RefParameter",
    }
}

fn is_not_blank(s: &str) -> bool {
    !s.chars().all(char::is_whitespace)
}

/// `new BigDecimal(String)`
fn parse_java_big_decimal(s: &str) -> JResult<BigDecimal> {
    BigDecimal::parse(s).map_err(|_| crate::v2::property_builder::number_format_bigdecimal(s))
}

/// `Integer.parseInt(s)`'s `NumberFormatException`
fn number_format_for_input(s: &str) -> JavaException {
    JavaException::new("java.lang.NumberFormatException", format!("For input string: \"{s}\""))
}

/// `(Boolean) value`
fn cast_boolean(v: &JavaObject) -> JResult<bool> {
    match v {
        JavaObject::Boolean(b) => Ok(*b),
        other => Err(crate::v2::property_builder::bootstrap_class_cast(object_class(other), "java.lang.Boolean")),
    }
}

fn object_class(o: &JavaObject) -> &'static str {
    match o {
        JavaObject::String(_) => "java.lang.String",
        JavaObject::Integer(_) => "java.lang.Integer",
        JavaObject::Long(_) => "java.lang.Long",
        JavaObject::Double(_) => "java.lang.Double",
        JavaObject::BigDecimal(_) => "java.math.BigDecimal",
        JavaObject::BigInteger(_) => "java.math.BigInteger",
        JavaObject::Map(_) => "java.util.LinkedHashMap",
        JavaObject::List(_) => "java.util.ArrayList",
        _ => "java.lang.Object",
    }
}

/// `schema.addProperties(key, value)` where the value may be null
fn add_property_nullable(schema: &Schema, key: &str, s: Option<Schema>) {
    match s {
        Some(s) => {
            schema.add_properties(key, s);
        }
        None => {
            let props = { schema.write().properties.get_or_insert_with(JMap::new).clone() };
            props.put_null_value(key);
        }
    }
}

/// `formSchema.addProperty(name, schema)` where the name may be null (a null key in the property map)
fn add_property_nullable_key(schema: &Schema, key: Option<&str>, s: Schema) {
    match key {
        Some(k) => {
            schema.add_property(k, s);
        }
        None => {
            let props = { schema.write().properties.get_or_insert_with(JMap::new).clone() };
            props.put_null_key(s);
        }
    }
}

/// `convert(Map<String, Object> vendorExtensions)`: null or empty maps are returned as they are.
fn convert_extensions(m: Option<&v2::VendorExtensions>) -> Option<JMap<JavaObject>> {
    let m = m?;
    if m.is_empty() {
        return Some(m.clone());
    }
    let result = JMap::with_kind(MapKind::Linked);
    for (k, v) in m.entries() {
        if !STRIPPED_EXTENSION_KEYS.contains(&k.as_str()) {
            result.put(k, v);
        }
    }
    Some(result)
}

fn rewrite_definition_ref(p: &PropertyRef) -> JResult<()> {
    let r = p.borrow().get_ref().ok_or_else(npe)?;
    if let Some(rest) = r.strip_prefix("#/definitions") {
        p.borrow_mut().set_ref(&format!("#/components/schemas{rest}"));
    }
    Ok(())
}

fn convert_security_scheme(d: &v2::SecuritySchemeDefinition) -> SecurityScheme {
    let s = match d.type_.as_deref() {
        Some("basic") => {
            let s = SecurityScheme::new();
            s.set_type(SecuritySchemeType::Http);
            s.set_scheme("basic");
            s
        }
        Some("apiKey") => {
            let s = SecurityScheme::new();
            s.set_type(SecuritySchemeType::ApiKey);
            s.set_name(d.name.clone());
            // `SecurityScheme.In.valueOf(in.toString())`: the constant names match
            s.set_in(d.in_.map(|i| match i {
                v2::In::Header => SecuritySchemeIn::Header,
                v2::In::Query => SecuritySchemeIn::Query,
            }));
            s
        }
        Some("oauth2") => convert_oauth2(d),
        _ => SecurityScheme::new(),
    };
    s.set_description(d.description.clone());
    s.set_extensions(convert_extensions(Some(&d.vendor_extensions)));
    s
}

fn convert_oauth2(d: &v2::SecuritySchemeDefinition) -> SecurityScheme {
    let s = SecurityScheme::new();
    let flows = OAuthFlows::new();
    let flow = OAuthFlow::new();
    s.set_type(SecuritySchemeType::Oauth2);
    match d.flow.as_deref() {
        Some("implicit") => {
            flow.set_authorization_url(d.authorization_url.clone());
            flows.set_implicit(flow.clone());
        }
        Some("password") => {
            flow.set_token_url(d.token_url.clone());
            flows.set_password(flow.clone());
        }
        Some("application") => {
            flow.set_token_url(d.token_url.clone());
            flows.set_client_credentials(flow.clone());
        }
        Some("accessCode") => {
            flow.set_authorization_url(d.authorization_url.clone());
            flow.set_token_url(d.token_url.clone());
            flows.set_authorization_code(flow.clone());
        }
        _ => {}
    }
    let scopes = Scopes::new();
    if let Some(sc) = &d.scopes {
        for (k, v) in sc {
            scopes_add_nullable(&scopes, k, v.as_deref());
        }
    }
    flow.set_scopes(scopes);
    s.set_flows(flows);
    s
}

/// `scopes.addString(k, v)` where the description may be null
fn scopes_add_nullable(scopes: &Scopes, k: &str, v: Option<&str>) {
    // A null description is not added: the Scopes map (String values) cannot hold it, and Json.mapper() omits
    // null map values when writing.
    if let Some(v) = v {
        scopes.add_string(k, v);
    }
}

fn convert_tags(tags: &[v2::Tag]) -> JList<Tag> {
    let out = JList::new();
    for t in tags {
        let v3 = Tag::new();
        v3.set_description(t.description.clone());
        v3.set_name(t.name.clone());
        if let Some(d) = &t.external_docs {
            v3.set_external_docs(convert_external_docs(d));
        }
        if let Some(e) = convert_extensions(Some(&t.vendor_extensions)) {
            v3.set_extensions(e);
        }
        out.add(v3);
    }
    out
}

fn convert_external_docs(d: &v2::ExternalDocs) -> ExternalDocumentation {
    let e = ExternalDocumentation::new();
    e.set_url(d.url.clone());
    e.set_description(d.description.clone());
    if let Some(x) = convert_extensions(Some(&d.vendor_extensions)) && x.size() > 0 {
        e.set_extensions(x);
    }
    e
}

fn convert_servers(schemes: Option<&[Option<v2::Scheme>]>, host: Option<&str>, base_path: Option<&str>) -> JResult<JList<Server>> {
    let servers = JList::new();
    let mut base_url = match base_path {
        Some(b) if !b.is_empty() => b.to_string(),
        _ => "/".to_string(),
    };
    if let Some(h) = host && !h.is_empty() {
        base_url = format!("{h}{base_url}");
    }
    if !base_url.starts_with('/') && schemes.is_some_and(|s| !s.is_empty()) {
        for scheme in schemes.unwrap() {
            let scheme = scheme.ok_or_else(npe)?;
            let s = Server::new();
            s.set_url(format!("{}://{base_url}", scheme.to_value()));
            servers.add(s);
        }
    } else {
        if !base_url.starts_with('/') && base_url != "/" {
            base_url = format!("//{base_url}");
        }
        let s = Server::new();
        s.set_url(base_url);
        servers.add(s);
    }
    Ok(servers)
}

/// `convert(io.swagger.models.Info)`
pub fn convert_info(i: &v2::Info) -> Info {
    let info = Info::new();
    info.set_contact(i.contact.as_ref().map(|c| {
        let x = Contact::new();
        x.set_url(c.url.clone());
        x.set_name(c.name.clone());
        x.set_email(c.email.clone());
        x
    }));
    info.set_description(i.description.clone());
    info.set_license(i.license.as_ref().map(|l| {
        let x = License::new();
        x.set_extensions(convert_extensions(Some(&l.vendor_extensions)));
        x.set_name(l.name.clone());
        x.set_url(l.url.clone());
        x
    }));
    info.set_terms_of_service(i.terms_of_service.clone());
    info.set_title(i.title.clone());
    info.set_version(i.version.clone());
    info.set_extensions(convert_extensions(Some(&i.vendor_extensions)));
    info
}

/// `convertExamples(examples, content)`
fn convert_examples(examples: Option<Vec<(String, JavaObject)>>, content: &Content) -> Content {
    if let Some(examples) = examples {
        for (k, v) in examples {
            let mt = match content.get(&k) {
                Some(m) => m,
                None => {
                    let m = MediaType::new();
                    content.add_media_type(&k, m.clone());
                    m
                }
            };
            mt.set_example(v);
        }
    }
    content.clone()
}

fn convert_file_schema(schema: Schema) -> Schema {
    if schema.get_type().as_deref() == Some("file") {
        schema.set_type("string");
        schema.set_format("binary");
    }
    schema
}

fn convert_header(p: Option<&PropertyRef>) -> JResult<Header> {
    let p = p.ok_or_else(npe)?;
    let schema = convert_property(Some(p))?.ok_or_else(npe)?;
    schema.set_description(None::<String>);
    let h = Header::new();
    h.set_description(p.borrow().description.clone());
    h.set_schema(schema);
    Ok(h)
}

/// `convert(SerializableParameter)` (form parameters)
fn convert_serializable(sp: &v2::Parameter) -> JResult<Schema> {
    if !sp.kind.is_serializable() {
        return Err(JavaException::class_cast(&format!("io.swagger.models.parameters.{}", v2_parameter_class(sp.kind)), "io.swagger.models.parameters.SerializableParameter"));
    }
    let schema = match sp.type_.as_deref() {
        Some("file") => Schema::of(SchemaKind::File),
        Some("array") => {
            let a = Schema::array_schema();
            if sp.items.is_some() {
                a.set_items(convert_property(sp.items.as_ref())?);
            }
            a
        }
        _ => match primitive_type_create(sp.type_.as_deref(), sp.format.as_deref()) {
            Some(s) => s,
            None => match primitive_type_create(sp.type_.as_deref(), None) {
                Some(s) => {
                    s.set_format(sp.format.clone());
                    s
                }
                None => {
                    let s = Schema::new();
                    s.set_type(sp.type_.clone());
                    s.set_format(sp.format.clone());
                    s
                }
            },
        },
    };
    schema.set_description(sp.description.clone());
    schema.set_read_only(sp.read_only);
    schema.set_enum(sp.enum_.as_ref().map(|e| JList::from_vec(e.iter().map(|s| JavaObject::string(s.as_str())).collect())));
    if sp.max_items.is_some() {
        schema.set_max_items(sp.max_items);
    }
    if sp.min_items.is_some() {
        schema.set_min_items(sp.min_items);
    }
    if sp.unique_items.is_some() {
        schema.set_unique_items(sp.unique_items);
    }
    schema.set_maximum(sp.maximum.clone());
    schema.set_exclusive_maximum(sp.exclusive_maximum);
    schema.set_minimum(sp.minimum.clone());
    schema.set_exclusive_minimum(sp.exclusive_minimum);
    schema.set_min_length(sp.min_length);
    schema.set_max_length(sp.max_length);
    schema.set_name(sp.name.clone());
    if let Some(ex) = sp.vendor_extensions.get("x-example") && !ex.is_null() {
        schema.set_example(ex);
    }
    if let Some(n) = sp.vendor_extensions.get("x-nullable") && !n.is_null() {
        schema.set_nullable(cast_boolean(&n)?);
    }
    schema.set_extensions(convert_extensions(Some(&sp.vendor_extensions)));
    if let Some(m) = &sp.multiple_of {
        schema.set_multiple_of(parse_java_big_decimal(&m.to_java_string())?);
    }
    schema.set_pattern(sp.pattern.clone());
    if let Some(d) = sp.get_default() {
        schema.set_default(d);
    }
    Ok(schema)
}

/// `PrimitiveType.fromTypeAndFormat(type, format)` followed by `createProperty()`
fn primitive_type_create(t: Option<&str>, f: Option<&str>) -> Option<Schema> {
    let blank = |s: Option<&str>| s.is_none_or(|s| s.chars().all(char::is_whitespace));
    if !blank(t) && t == Some("object") {
        return None;
    }
    let key = format!("{}_{}", if blank(t) { "" } else { t.unwrap() }, if blank(f) { "" } else { f.unwrap() });
    let name = match key.as_str() {
        "integer_int32" | "integer_" => "integer",
        "integer_int64" => "long",
        "number_" => "number",
        "number_float" => "float",
        "number_double" => "double",
        "string_" => "string",
        "string_byte" => "byte",
        "string_email" => "email",
        "string_binary" => "binary",
        "string_uri" => "uri",
        "string_url" => "url",
        "string_uuid" => "uuid",
        "string_date" => "date",
        "string_date-time" => "date-time",
        "string_partial-time" => "partial-time",
        "boolean_" => "boolean",
        "object_" => "object",
        _ => return None,
    };
    let binary_as_string = || {
        crate::java::system::get_property("binary-string-conversion").as_deref() == Some("BINARY_STRING_CONVERSION_STRING_SCHEMA")
            || std::env::var("binary-string-conversion").ok().as_deref() == Some("BINARY_STRING_CONVERSION_STRING_SCHEMA")
    };
    Some(match name {
        "string" => Schema::string_schema(),
        "boolean" => Schema::boolean_schema(),
        "byte" => {
            if binary_as_string() {
                Schema::string_schema().format("byte")
            } else {
                Schema::of(SchemaKind::ByteArray)
            }
        }
        "binary" => {
            if binary_as_string() {
                Schema::string_schema().format("binary")
            } else {
                Schema::of(SchemaKind::Binary)
            }
        }
        "uri" => Schema::string_schema().format("uri"),
        "url" => Schema::string_schema().format("url"),
        "email" => Schema::string_schema().format("email"),
        "uuid" => Schema::of(SchemaKind::Uuid),
        "integer" => Schema::integer_schema(),
        "long" => Schema::integer_schema().format("int64"),
        "float" => Schema::number_schema().format("float"),
        "double" => Schema::number_schema().format("double"),
        "number" => Schema::number_schema(),
        "date" => Schema::of(SchemaKind::Date),
        "date-time" => Schema::of(SchemaKind::DateTime),
        "partial-time" => Schema::string_schema().format("partial-time"),
        "object" => {
            let s = Schema::new();
            // `PrimitiveType.explicitObjectType` (env first, then the system property; unset means true)
            let explicit = std::env::var("explicit-object-schema").ok().or_else(|| crate::java::system::get_property("explicit-object-schema")).map(|v| v.eq_ignore_ascii_case("true"));
            if explicit != Some(false) {
                s.set_type("object");
            }
            s
        }
        _ => return None,
    })
}

/// `convert(Property)`; `Ok(None)` for a null property.
pub fn convert_property(p: Option<&PropertyRef>) -> JResult<Option<Schema>> {
    let Some(p) = p else { return Ok(None) };
    if let Some(b) = p.borrow().boolean_value {
        let s = Schema::new();
        s.set_boolean_schema_value(b);
        return Ok(Some(s));
    }
    let kind = p.borrow().kind;
    let result = match kind {
        PropertyKind::Ref => {
            rewrite_definition_ref(p)?;
            let s = Schema::new();
            s.set_ref(p.borrow().get_ref());
            s
        }
        PropertyKind::Array => {
            let node = property_to_tree_with(&p.borrow(), false).map_err(jackson_err)?;
            let a = bean_schema(SchemaKind::Array, &node, Mapper::Json30).map_err(jackson_err)?;
            let items = p.borrow().items.clone();
            a.set_items(convert_property(items.as_ref())?);
            let (max, min, uniq) = { let x = p.borrow(); (x.max_items, x.min_items, x.unique_items) };
            if max.is_some() {
                a.set_max_items(max);
            }
            if min.is_some() {
                a.set_min_items(min);
            }
            if uniq == Some(true) {
                a.set_unique_items(uniq);
            }
            a
        }
        PropertyKind::File => {
            let node = property_to_tree_with(&p.borrow(), false).map_err(jackson_err)?;
            bean_schema(SchemaKind::File, &node, Mapper::Json30).map_err(jackson_err)?
        }
        _ => {
            let node = property_to_tree_with(&p.borrow(), true).map_err(jackson_err)?;
            let result = schema_from_tree_nullable(&node, Mapper::Json30).map_err(jackson_err)?.ok_or_else(npe)?;
            let pb = p.borrow().clone();
            if let Some(e) = &pb.example && !e.is_null() {
                result.set_example(e.clone());
            }
            if pb.type_.as_deref() == Some("object") && result.get_properties().is_some_and(|m| m.size() > 0) {
                if kind != PropertyKind::Object {
                    return Err(JavaException::class_cast(kind.class_name(), "io.swagger.models.properties.ObjectProperty"));
                }
                let props: JMap<Schema> = JMap::with_kind(MapKind::Linked);
                if let Some(pp) = &pb.properties {
                    for (k, v) in pp.entries() {
                        match convert_property(Some(&v))? {
                            Some(s) => {
                                props.put(k, s);
                            }
                            None => {
                                props.put_null_value(&k);
                            }
                        }
                    }
                } else {
                    return Err(npe());
                }
                result.set_properties(props);
            }
            if kind == PropertyKind::Map {
                let ap = pb.additional_properties.clone().ok_or_else(npe)?;
                let b = ap.borrow().boolean_value;
                match b {
                    Some(b) => result.set_additional_properties(AdditionalProperties::Bool(b)),
                    None => result.set_additional_properties(convert_property(Some(&ap))?.map(AdditionalProperties::Schema)),
                }
                result.set_min_properties(pb.min_properties);
                result.set_max_properties(pb.max_properties);
            }
            if kind.is_numeric() {
                result.set_exclusive_maximum(pb.exclusive_maximum);
                result.set_exclusive_minimum(pb.exclusive_minimum);
            }
            if kind.is_string_property() {
                result.set_min_length(pb.min_length);
                result.set_max_length(pb.max_length);
                result.set_pattern(pb.pattern.clone());
            }
            result
        }
    };
    let ext = p.borrow().vendor_extensions.clone();
    if let Some(n) = ext.get("x-nullable") && !n.is_null() {
        result.set_nullable(cast_boolean(&n)?);
    }
    result.set_extensions(convert_extensions(Some(&ext)));
    Ok(Some(result))
}

// ---------------------------------------------------------------------------------------------------------
// SwaggerInventory

/// `io.swagger.v3.parser.converter.SwaggerInventory`
#[derive(Default)]
pub struct SwaggerInventory {
    /// The models found, in visiting order.
    pub models: Vec<ModelRef>,
    /// may hold nulls (an `ArrayProperty` without items)
    pub properties: Vec<Option<PropertyRef>>,
}

impl SwaggerInventory {
    /// An empty inventory.
    pub fn new() -> SwaggerInventory {
        SwaggerInventory::default()
    }

    /// `process(swagger)`: collects the models and properties of every path and definition of `swagger`.
    pub fn process(mut self, swagger: &v2::Shared<v2::Swagger>) -> JResult<SwaggerInventory> {
        let sw = swagger.borrow().clone();
        if let Some(paths) = &sw.paths {
            for path in paths.values() {
                self.process_path(path)?;
            }
        }
        if let Some(defs) = &sw.definitions {
            for m in defs.values() {
                self.process_model(m);
            }
        }
        Ok(self)
    }

    fn process_path(&mut self, path: &v2::PathRef) -> JResult<()> {
        let (params, ops) = { let p = path.borrow(); (p.parameters.clone(), p.get_operations()) };
        if let Some(params) = params {
            for p in params {
                self.process_parameter(&p);
            }
        }
        for op in ops {
            let (params, responses) = { let o = op.borrow(); (o.parameters.clone(), o.responses.clone()) };
            if let Some(params) = params {
                for p in params {
                    self.process_parameter(&p);
                }
            }
            if let Some(responses) = responses {
                for r in responses.values() {
                    let Some(r) = r else { return Err(npe()) };
                    // `response.getSchema()`: the deprecated property view of the response schema
                    let schema = r.borrow().schema.clone();
                    if let Some(schema) = schema && let Some(p) = model_to_property(&schema)? {
                        self.process_property(Some(&p));
                    }
                }
            }
        }
        Ok(())
    }

    fn process_parameter(&mut self, p: &ParameterRef) {
        let schema = { let x = p.borrow(); if x.kind == V2ParameterKind::Body { x.schema.clone() } else { None } };
        if let Some(s) = schema {
            self.process_model(&s);
        }
    }

    fn process_model(&mut self, m: &ModelRef) {
        self.models.push(m.clone());
        let (kind, props, all_of) = { let x = m.borrow(); (x.kind, x.properties.clone(), x.all_of.clone()) };
        match kind {
            ModelKind::Impl | ModelKind::Array => {
                if let Some(props) = props {
                    for p in props.values() {
                        self.process_property(Some(&p));
                    }
                }
            }
            ModelKind::Composed => {
                for x in all_of {
                    self.process_model(&x);
                }
            }
            _ => {}
        }
    }

    fn process_property(&mut self, p: Option<&PropertyRef>) {
        self.properties.push(p.cloned());
        let Some(p) = p else { return };
        let (kind, items, props) = { let x = p.borrow(); (x.kind, x.items.clone(), x.properties.clone()) };
        match kind {
            PropertyKind::Array => self.process_property(items.as_ref()),
            PropertyKind::Object => {
                if let Some(props) = props {
                    for x in props.values() {
                        self.process_property(Some(&x));
                    }
                }
            }
            _ => {}
        }
    }
}

/// `Yaml.pretty(openAPI)`
fn yaml_pretty(o: &OpenAPI) -> Option<String> {
    crate::models::yaml_pretty(o)
}

trait ComponentMaps {
    fn get_schemas_or_create(&self) -> JMap<Schema>;
    fn get_parameters_or_create(&self) -> JMap<Parameter>;
    fn get_request_bodies_or_create(&self) -> JMap<RequestBody>;
    fn get_responses_or_create(&self) -> JMap<ApiResponse>;
    fn get_security_schemes_or_create(&self) -> JMap<SecurityScheme>;
}

impl ComponentMaps for Components {
    fn get_schemas_or_create(&self) -> JMap<Schema> {
        self.write().schemas.get_or_insert_with(JMap::new).clone()
    }
    fn get_parameters_or_create(&self) -> JMap<Parameter> {
        self.write().parameters.get_or_insert_with(JMap::new).clone()
    }
    fn get_request_bodies_or_create(&self) -> JMap<RequestBody> {
        self.write().request_bodies.get_or_insert_with(JMap::new).clone()
    }
    fn get_responses_or_create(&self) -> JMap<ApiResponse> {
        self.write().responses.get_or_insert_with(JMap::new).clone()
    }
    fn get_security_schemes_or_create(&self) -> JMap<SecurityScheme> {
        self.write().security_schemes.get_or_insert_with(JMap::new).clone()
    }
}
