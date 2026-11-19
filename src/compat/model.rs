//! The Swagger 1.2 model of swagger-compat-spec-parser (resource listings and API declarations), read by
//! Jackson bean deserialization through swagger-core 1.x's `Json.mapper()`: unknown members go to
//! `extraFields` (`@JsonAnySetter`), enums use their lower-casing `forValue` creators, and maps declared
//! as `HashMap` keep Java's hash iteration order.

use crate::java::{JResult, JavaException};
use indexmap::IndexMap;
use jackson::JsonNode;

fn iae(msg: impl Into<String>) -> JavaException {
    JavaException::illegal_argument(msg)
}

fn fields(v: &JsonNode) -> Vec<(String, JsonNode)> {
    v.as_object().map(|o| o.iter().map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default()
}

/// true for an object, false for JSON null, an error for anything else
fn bean_object(v: &JsonNode, class: &str) -> JResult<bool> {
    match v {
        JsonNode::Null => Ok(false),
        JsonNode::Object(_) => Ok(true),
        _ => Err(iae(format!("Cannot construct instance of `{class}`"))),
    }
}

fn s(v: &JsonNode) -> JResult<Option<String>> {
    match v {
        JsonNode::Null => Ok(None),
        JsonNode::Object(_) | JsonNode::Array(_) => Err(iae("Cannot deserialize value of type `java.lang.String`")),
        _ => Ok(Some(v.as_text())),
    }
}

fn b(v: &JsonNode) -> JResult<Option<bool>> {
    crate::models::deserialize::de_bool(v).map_err(|e| iae(e.message))
}

fn i(v: &JsonNode) -> JResult<Option<i32>> {
    crate::models::deserialize::de_int(v).map_err(|e| iae(e.message))
}

fn strings(v: &JsonNode) -> JResult<Option<Vec<String>>> {
    match v {
        JsonNode::Null => Ok(None),
        JsonNode::Array(a) => {
            let mut out = Vec::new();
            for e in a.iter() {
                if !e.is_null() {
                    out.push(s(e)?.unwrap_or_default());
                }
            }
            Ok(Some(out))
        }
        _ => Err(iae("Cannot deserialize value of type `java.util.ArrayList<java.lang.String>`")),
    }
}

fn list<T>(v: &JsonNode, mut f: impl FnMut(&JsonNode) -> JResult<Option<T>>) -> JResult<Option<Vec<Option<T>>>> {
    match v {
        JsonNode::Null => Ok(None),
        JsonNode::Array(a) => {
            let mut out = Vec::new();
            for e in a.iter() {
                out.push(f(e)?);
            }
            Ok(Some(out))
        }
        _ => Err(iae("Cannot deserialize value of type `java.util.ArrayList`")),
    }
}

/// A `Map`'s entries in document order, a value `None` for a JSON null.
type Entries<T> = Vec<(String, Option<T>)>;

/// A `Map` property (Jackson builds a `LinkedHashMap`, so document order is kept even where the field is
/// initialised with a `HashMap`)
fn hash_map<T: Clone>(v: &JsonNode, mut f: impl FnMut(&JsonNode) -> JResult<Option<T>>) -> JResult<Option<Entries<T>>> {
    match v {
        JsonNode::Null => Ok(None),
        JsonNode::Object(o) => {
            let mut out = Vec::new();
            for (k, e) in o.iter() {
                out.push((k.clone(), f(e)?));
            }
            Ok(Some(out))
        }
        _ => Err(iae("Cannot deserialize value of type `java.util.LinkedHashMap`")),
    }
}

/// An enum with a `@JsonCreator forValue(String)` looking up the lower-cased text (`null` if unknown)
pub fn enum_value(v: &JsonNode, names: &[&'static str]) -> JResult<Option<&'static str>> {
    let Some(t) = s(v)? else { return Ok(None) };
    let l = t.to_lowercase();
    Ok(names.iter().copied().find(|n| **n == l))
}

pub const FORMATS: [&str; 7] = ["int32", "int64", "float", "double", "byte", "date", "date-time"];
pub const METHODS: [&str; 7] = ["get", "put", "post", "delete", "patch", "options", "head"];
pub const PARAM_TYPES: [&str; 5] = ["path", "query", "header", "body", "form"];

/// `AuthorizationScope`
#[derive(Clone, Debug, Default)]
pub struct AuthorizationScope {
    pub scope: Option<String>,
    pub description: Option<String>,
}

fn authorization_scope(v: &JsonNode) -> JResult<Option<AuthorizationScope>> {
    if !bean_object(v, "io.swagger.models.AuthorizationScope")? {
        return Ok(None);
    }
    let mut a = AuthorizationScope::default();
    for (k, f) in fields(v) {
        match k.as_str() {
            "scope" => a.scope = s(&f)?,
            "description" => a.description = s(&f)?,
            _ => {}
        }
    }
    Ok(Some(a))
}

pub type ScopeLists = Vec<(String, Option<Vec<Option<AuthorizationScope>>>)>;

fn scope_lists(v: &JsonNode) -> JResult<Option<ScopeLists>> {
    hash_map(v, |e| list(e, authorization_scope))
}

/// `Items` (a `TypedObject`)
#[derive(Clone, Debug, Default)]
pub struct Items {
    pub type_: Option<String>,
    pub ref_: Option<String>,
    pub format: Option<&'static str>,
    pub extra: IndexMap<String, JsonNode>,
}

fn items(v: &JsonNode) -> JResult<Option<Items>> {
    if !bean_object(v, "io.swagger.models.apideclaration.Items")? {
        return Ok(None);
    }
    let mut x = Items::default();
    for (k, f) in fields(v) {
        match k.as_str() {
            "type" => x.type_ = s(&f)?,
            "$ref" => x.ref_ = s(&f)?,
            "format" => x.format = enum_value(&f, &FORMATS)?,
            _ => {
                x.extra.insert(k, f);
            }
        }
    }
    Ok(Some(x))
}

/// The `TypedObject` / `ExtendedTypedObject` fields
#[derive(Clone, Debug, Default)]
pub struct Typed {
    pub type_: Option<String>,
    pub ref_: Option<String>,
    pub format: Option<&'static str>,
    pub default_value: Option<String>,
    pub enum_values: Option<Vec<String>>,
    pub minimum: Option<String>,
    pub maximum: Option<String>,
    pub items: Option<Items>,
    pub extra: IndexMap<String, JsonNode>,
}

/// Sets an `ExtendedTypedObject` field; `false` when `k` is none of them.
fn typed_field(t: &mut Typed, k: &str, f: &JsonNode) -> JResult<bool> {
    match k {
        "type" => t.type_ = s(f)?,
        "$ref" => t.ref_ = s(f)?,
        "format" => t.format = enum_value(f, &FORMATS)?,
        "defaultValue" => t.default_value = s(f)?,
        "enum" => t.enum_values = strings(f)?,
        "minimum" => t.minimum = s(f)?,
        "maximum" => t.maximum = s(f)?,
        "items" => t.items = items(f)?,
        "uniqueItems" => {
            b(f)?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

/// `io.swagger.models.apideclaration.Parameter`
#[derive(Clone, Debug, Default)]
pub struct Parameter12 {
    pub typed: Typed,
    pub param_type: Option<&'static str>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub required: Option<bool>,
    pub allow_multiple: Option<bool>,
}

fn parameter12(v: &JsonNode) -> JResult<Option<Parameter12>> {
    if !bean_object(v, "io.swagger.models.apideclaration.Parameter")? {
        return Ok(None);
    }
    let mut p = Parameter12::default();
    for (k, f) in fields(v) {
        if typed_field(&mut p.typed, &k, &f)? {
            continue;
        }
        match k.as_str() {
            "paramType" => p.param_type = enum_value(&f, &PARAM_TYPES)?,
            "name" => p.name = s(&f)?,
            "description" => p.description = s(&f)?,
            "required" => p.required = b(&f)?,
            "allowMultiple" => p.allow_multiple = b(&f)?,
            _ => {
                p.typed.extra.insert(k, f);
            }
        }
    }
    Ok(Some(p))
}

/// `ResponseMessage`
#[derive(Clone, Debug, Default)]
pub struct ResponseMessage {
    pub code: Option<i32>,
    pub message: Option<String>,
    pub response_model: Option<String>,
}

fn response_message(v: &JsonNode) -> JResult<Option<ResponseMessage>> {
    if !bean_object(v, "io.swagger.models.apideclaration.ResponseMessage")? {
        return Ok(None);
    }
    let mut r = ResponseMessage::default();
    for (k, f) in fields(v) {
        match k.as_str() {
            "code" => r.code = i(&f)?,
            "message" => r.message = s(&f)?,
            "responseModel" => r.response_model = s(&f)?,
            _ => {}
        }
    }
    Ok(Some(r))
}

/// `io.swagger.models.apideclaration.Operation`
#[derive(Clone, Debug, Default)]
pub struct Operation12 {
    pub typed: Typed,
    pub method: Option<&'static str>,
    pub summary: Option<String>,
    pub notes: Option<String>,
    pub nickname: Option<String>,
    pub authorizations: Option<ScopeLists>,
    pub parameters: Option<Vec<Option<Parameter12>>>,
    pub response_messages: Option<Vec<Option<ResponseMessage>>>,
    pub produces: Option<Vec<String>>,
    pub consumes: Option<Vec<String>>,
}

fn operation12(v: &JsonNode) -> JResult<Option<Operation12>> {
    if !bean_object(v, "io.swagger.models.apideclaration.Operation")? {
        return Ok(None);
    }
    let mut o = Operation12 {
        authorizations: Some(Vec::new()),
        parameters: Some(Vec::new()),
        response_messages: Some(Vec::new()),
        produces: Some(Vec::new()),
        consumes: Some(Vec::new()),
        ..Default::default()
    };
    for (k, f) in fields(v) {
        if typed_field(&mut o.typed, &k, &f)? {
            continue;
        }
        match k.as_str() {
            "method" => o.method = enum_value(&f, &METHODS)?,
            "summary" => o.summary = s(&f)?,
            "notes" => o.notes = s(&f)?,
            "nickname" => o.nickname = s(&f)?,
            "authorizations" => o.authorizations = scope_lists(&f)?,
            "parameters" => o.parameters = list(&f, parameter12)?,
            "responseMessages" => o.response_messages = list(&f, response_message)?,
            "produces" => o.produces = strings(&f)?,
            "consumes" => o.consumes = strings(&f)?,
            "deprecated" => {
                b(&f)?;
            }
            _ => {
                o.typed.extra.insert(k, f);
            }
        }
    }
    Ok(Some(o))
}

/// `io.swagger.models.apideclaration.Api`
#[derive(Clone, Debug, Default)]
pub struct Api12 {
    pub path: Option<String>,
    pub operations: Option<Vec<Option<Operation12>>>,
}

fn api12(v: &JsonNode) -> JResult<Option<Api12>> {
    if !bean_object(v, "io.swagger.models.apideclaration.Api")? {
        return Ok(None);
    }
    let mut a = Api12 { operations: Some(Vec::new()), ..Default::default() };
    for (k, f) in fields(v) {
        match k.as_str() {
            "path" => a.path = s(&f)?,
            "description" => {
                s(&f)?;
            }
            "operations" => a.operations = list(&f, operation12)?,
            _ => {}
        }
    }
    Ok(Some(a))
}

/// `ModelProperty`
#[derive(Clone, Debug, Default)]
pub struct ModelProperty {
    pub typed: Typed,
    pub description: Option<String>,
}

fn model_property(v: &JsonNode) -> JResult<Option<ModelProperty>> {
    if !bean_object(v, "io.swagger.models.apideclaration.ModelProperty")? {
        return Ok(None);
    }
    let mut p = ModelProperty::default();
    for (k, f) in fields(v) {
        if typed_field(&mut p.typed, &k, &f)? {
            continue;
        }
        match k.as_str() {
            "description" => p.description = s(&f)?,
            _ => {
                p.typed.extra.insert(k, f);
            }
        }
    }
    Ok(Some(p))
}

/// `io.swagger.models.apideclaration.Model`
#[derive(Clone, Debug, Default)]
pub struct Model12 {
    pub id: Option<String>,
    pub description: Option<String>,
    pub required: Option<Vec<String>>,
    pub properties: Option<Vec<(String, Option<ModelProperty>)>>,
    pub discriminator: Option<String>,
}

fn model12(v: &JsonNode) -> JResult<Option<Model12>> {
    if !bean_object(v, "io.swagger.models.apideclaration.Model")? {
        return Ok(None);
    }
    let mut m = Model12::default();
    for (k, f) in fields(v) {
        match k.as_str() {
            "id" => m.id = s(&f)?,
            "description" => m.description = s(&f)?,
            "required" => m.required = strings(&f)?,
            // `Map<String, ModelProperty>` (no initializer: a LinkedHashMap from Jackson)
            "properties" => {
                m.properties = match &f {
                    JsonNode::Null => None,
                    JsonNode::Object(o) => {
                        let mut out = Vec::new();
                        for (pk, pv) in o.iter() {
                            out.push((pk.clone(), model_property(pv)?));
                        }
                        Some(out)
                    }
                    _ => return Err(iae("Cannot deserialize value of type `java.util.LinkedHashMap`")),
                };
            }
            "discriminator" => m.discriminator = s(&f)?,
            "subTypes" => {
                strings(&f)?;
            }
            _ => {}
        }
    }
    Ok(Some(m))
}

/// `io.swagger.models.apideclaration.ApiDeclaration`
#[derive(Clone, Debug, Default)]
pub struct ApiDeclaration {
    pub base_path: Option<String>,
    pub resource_path: Option<String>,
    pub apis: Option<Vec<Option<Api12>>>,
    pub models: Option<Vec<(String, Option<Model12>)>>,
    pub produces: Option<Vec<String>>,
    pub consumes: Option<Vec<String>>,
}

pub fn api_declaration(v: &JsonNode) -> JResult<Option<ApiDeclaration>> {
    if !bean_object(v, "io.swagger.models.apideclaration.ApiDeclaration")? {
        return Ok(None);
    }
    let mut d = ApiDeclaration { apis: Some(Vec::new()), models: Some(Vec::new()), produces: Some(Vec::new()), consumes: Some(Vec::new()), ..Default::default() };
    for (k, f) in fields(v) {
        match k.as_str() {
            "swaggerVersion" | "apiVersion" => {
                s(&f)?;
            }
            "basePath" => d.base_path = s(&f)?,
            "resourcePath" => d.resource_path = s(&f)?,
            "apis" => d.apis = list(&f, api12)?,
            "models" => d.models = hash_map(&f, model12)?,
            "produces" => d.produces = strings(&f)?,
            "consumes" => d.consumes = strings(&f)?,
            "authorizations" => {
                scope_lists(&f)?;
            }
            _ => {}
        }
    }
    Ok(Some(d))
}

/// `ApiListingReference`
#[derive(Clone, Debug, Default)]
pub struct ApiListingReference {
    pub path: Option<String>,
    pub extra: IndexMap<String, JsonNode>,
}

/// `GrantTypes`: the implicit grant's login URL and the authorization code grant's token request / token
/// URLs (`None` for a missing grant or endpoint object, `Some(None)` for an endpoint without a URL)
#[derive(Clone, Debug, Default)]
pub struct GrantTypes {
    pub implicit: Option<Endpoint>,
    /// The token request endpoint's URL and the token endpoint's.
    pub authorization_code: Option<(Endpoint, Endpoint)>,
}

/// A grant's endpoint object: `None` when missing, `Some(None)` without a URL.
pub type Endpoint = Option<Option<String>>;

/// `Authorization` (polymorphic on its `type` member)
#[derive(Clone, Debug)]
pub enum Authorization {
    ApiKey { pass_as: Option<&'static str>, keyname: Option<String> },
    Basic,
    OAuth2 { scopes: Option<Vec<Option<AuthorizationScope>>>, grant_types: Option<GrantTypes> },
}

/// `ApiInfo`
#[derive(Clone, Debug, Default)]
pub struct ApiInfo {
    pub title: Option<String>,
    pub description: Option<String>,
    pub terms_of_service_url: Option<String>,
    pub contact: Option<String>,
    pub license: Option<String>,
    pub license_url: Option<String>,
}

/// `ResourceListing`
#[derive(Clone, Debug, Default)]
pub struct ResourceListing {
    pub swagger_version: Option<&'static str>,
    pub api_version: Option<String>,
    pub apis: Option<Vec<Option<ApiListingReference>>>,
    pub authorizations: Option<Vec<(String, Option<Authorization>)>>,
    pub info: Option<ApiInfo>,
    pub extra: IndexMap<String, JsonNode>,
}

fn url_bean(v: &JsonNode, class: &str) -> JResult<Option<Option<String>>> {
    if !bean_object(v, class)? {
        return Ok(None);
    }
    let mut url = None;
    for (k, f) in fields(v) {
        match k.as_str() {
            "url" => url = s(&f)?,
            "tokenName" | "clientIdName" | "clientSecretName" => {
                s(&f)?;
            }
            _ => {}
        }
    }
    Ok(Some(url))
}

fn grant_types(v: &JsonNode) -> JResult<Option<GrantTypes>> {
    if !bean_object(v, "io.swagger.models.resourcelisting.GrantTypes")? {
        return Ok(None);
    }
    let mut g = GrantTypes::default();
    for (k, f) in fields(v) {
        match k.as_str() {
            "implicit" => {
                g.implicit = if bean_object(&f, "io.swagger.models.resourcelisting.ImplicitGrant")? {
                    let mut login = None;
                    for (ik, iv) in fields(&f) {
                        match ik.as_str() {
                            "loginEndpoint" => login = url_bean(&iv, "io.swagger.models.resourcelisting.LoginEndpoint")?,
                            "tokenName" => {
                                s(&iv)?;
                            }
                            _ => {}
                        }
                    }
                    Some(login)
                } else {
                    None
                };
            }
            "authorization_code" => {
                g.authorization_code = if bean_object(&f, "io.swagger.models.resourcelisting.AuthorizationCodeGrant")? {
                    let (mut req, mut tok) = (None, None);
                    for (ak, av) in fields(&f) {
                        match ak.as_str() {
                            "tokenRequestEndpoint" => req = url_bean(&av, "io.swagger.models.resourcelisting.TokenRequestEndpoint")?,
                            "tokenEndpoint" => tok = url_bean(&av, "io.swagger.models.resourcelisting.TokenEndpoint")?,
                            _ => {}
                        }
                    }
                    Some((req, tok))
                } else {
                    None
                };
            }
            _ => {}
        }
    }
    Ok(Some(g))
}

fn authorization(v: &JsonNode) -> JResult<Option<Authorization>> {
    if v.is_null() {
        return Ok(None);
    }
    if !v.is_object() {
        return Err(iae("Cannot deserialize value of type `io.swagger.models.resourcelisting.Authorization`"));
    }
    let Some(t) = v.get("type") else {
        return Err(iae("Could not resolve subtype of [simple type, class io.swagger.models.resourcelisting.Authorization]: missing type id property 'type'"));
    };
    let mut auth = match t.as_text().as_str() {
        "apiKey" => Authorization::ApiKey { pass_as: None, keyname: None },
        "basicAuth" => Authorization::Basic,
        "oauth2" => Authorization::OAuth2 { scopes: Some(Vec::new()), grant_types: None },
        other => return Err(iae(format!("Could not resolve type id '{other}' as a subtype of `io.swagger.models.resourcelisting.Authorization`"))),
    };
    for (k, f) in fields(v) {
        match (&mut auth, k.as_str()) {
            (_, "type") => {}
            (Authorization::ApiKey { pass_as, .. }, "passAs") => *pass_as = enum_value(&f, &["header", "query"])?,
            (Authorization::ApiKey { keyname, .. }, "keyname") => *keyname = s(&f)?,
            (Authorization::OAuth2 { scopes, .. }, "scopes") => *scopes = list(&f, authorization_scope)?,
            (Authorization::OAuth2 { grant_types: g, .. }, "grantTypes") => *g = grant_types(&f)?,
            _ => {}
        }
    }
    Ok(Some(auth))
}

pub fn resource_listing(v: &JsonNode) -> JResult<Option<ResourceListing>> {
    if !bean_object(v, "io.swagger.models.resourcelisting.ResourceListing")? {
        return Ok(None);
    }
    let mut r = ResourceListing { apis: Some(Vec::new()), authorizations: Some(Vec::new()), ..Default::default() };
    for (k, f) in fields(v) {
        match k.as_str() {
            "swaggerVersion" => r.swagger_version = enum_value(&f, &["1.0", "1.1", "1.2"])?,
            "apiVersion" => r.api_version = s(&f)?,
            "apis" => {
                r.apis = list(&f, |e| {
                    if !bean_object(e, "io.swagger.models.resourcelisting.ApiListingReference")? {
                        return Ok(None);
                    }
                    let mut a = ApiListingReference::default();
                    for (ak, av) in fields(e) {
                        match ak.as_str() {
                            "path" => a.path = s(&av)?,
                            "description" => {
                                s(&av)?;
                            }
                            _ => {
                                a.extra.insert(ak, av);
                            }
                        }
                    }
                    Ok(Some(a))
                })?;
            }
            "authorizations" => r.authorizations = hash_map(&f, authorization)?,
            "info" => {
                r.info = if bean_object(&f, "io.swagger.models.resourcelisting.ApiInfo")? {
                    let mut i = ApiInfo::default();
                    for (ik, iv) in fields(&f) {
                        match ik.as_str() {
                            "title" => i.title = s(&iv)?,
                            "description" => i.description = s(&iv)?,
                            "termsOfServiceUrl" => i.terms_of_service_url = s(&iv)?,
                            "contact" => i.contact = s(&iv)?,
                            "license" => i.license = s(&iv)?,
                            "licenseUrl" => i.license_url = s(&iv)?,
                            _ => {}
                        }
                    }
                    Some(i)
                } else {
                    None
                };
            }
            _ => {
                r.extra.insert(k, f);
            }
        }
    }
    Ok(Some(r))
}
