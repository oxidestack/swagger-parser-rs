#!/usr/bin/env python3
"""Generates src/models/generated.rs: the swagger-core 2.2.53 v3 model classes (data, accessors, equality,
Jackson bean serialization in the recorded property order, bean deserialization).

Property orders come from reference/models/order.txt (recorded from the real Jackson mappers).
Hand-written behaviour (custom setters, custom serializers) lives in src/models/*.rs; fields listed in a
class's `custom` set get no generated accessors."""
import re, os, collections

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# ---- property orders ----
ORDER = {}
for line in open(os.path.join(ROOT, 'reference/models/order.txt')):
    line = line.strip()
    m = re.match(r'(\w+) (\w+) (\w+)(?: \[(.*)\])?', line)
    if not m:
        continue
    mapper, cls, ser, props = m.groups()
    if props is None:
        ORDER[(mapper, cls)] = None
        continue
    out = []
    for p in props.split(', '):
        name = re.sub(r'\{\w+\}', '', p)
        incl = re.search(r'<(\w+),', name)
        suppress = incl.group(1) if incl else 'true'
        name = re.sub(r'<.*>', '', name)
        anyget = '#AnyGetterWriter' in name
        name = name.replace('#AnyGetterWriter', '')
        out.append((name, anyget, suppress == 'true'))
    ORDER[(mapper, cls)] = out

# ---- class specs ----
# type codes: S String, B bool, I i32, BD BigDecimal, O JavaObject, F primitive flag (bool),
# M:X model, L:T list, MAP:T string-keyed map, SET string set, E:X enum, AP additionalProperties
EXT = ('extensions', 'MAP:O')

def F(*pairs):
    return [p for p in pairs]

CLASSES = collections.OrderedDict()
def cls(name, fields, **kw):
    CLASSES[name] = dict(fields=fields, **kw)

cls('OpenAPI', [('openapi','S'),('info','M:Info'),('externalDocs','M:ExternalDocumentation'),('servers','L:M:Server'),('security','L:M:SecurityRequirement'),('tags','L:M:Tag'),('paths','M:Paths'),('components','M:Components'),EXT,('jsonSchemaDialect','S'),('specVersion','E:SpecVersion'),('webhooks','MAP:M:PathItem')], ext='x', eq_skip={'specVersion'}, custom_new=True)
cls('Components', [('schemas','MAP:M:Schema'),('responses','MAP:M:ApiResponse'),('parameters','MAP:M:Parameter'),('examples','MAP:M:Example'),('requestBodies','MAP:M:RequestBody'),('headers','MAP:M:Header'),('securitySchemes','MAP:M:SecurityScheme'),('links','MAP:M:Link'),('callbacks','MAP:M:Callback'),EXT,('pathItems','MAP:M:PathItem')], ext='x')
cls('ExternalDocumentation', [('description','S'),('url','S'),EXT], ext='x')
cls('Operation', [('tags','L:S'),('summary','S'),('description','S'),('externalDocs','M:ExternalDocumentation'),('operationId','S'),('parameters','L:M:Parameter'),('requestBody','M:RequestBody'),('responses','M:ApiResponses'),('callbacks','MAP:M:Callback'),('deprecated','B'),('security','L:M:SecurityRequirement'),('servers','L:M:Server'),EXT], ext='x')
cls('PathItem', [('summary','S'),('description','S'),('get','M:Operation'),('put','M:Operation'),('post','M:Operation'),('delete','M:Operation'),('options','M:Operation'),('head','M:Operation'),('patch','M:Operation'),('trace','M:Operation'),('servers','L:M:Server'),('parameters','L:M:Parameter'),('$ref','S'),EXT], ext='x')
cls('Example', [('summary','S'),('description','S'),('value','O'),('externalValue','S'),('$ref','S'),EXT,('valueSetFlag','F')], ext='x', custom={'value','$ref'}, eq_skip={'valueSetFlag'})
cls('Header', [('description','S'),('$ref','S'),('required','B'),('deprecated','B'),('style','E:HeaderStyle'),('explode','B'),('schema','M:Schema'),('examples','MAP:M:Example'),('example','O'),('content','M:Content'),EXT], ext='x30', custom={'$ref'})
cls('Contact', [('name','S'),('url','S'),('email','S'),EXT], ext='x')
cls('Info', [('title','S'),('description','S'),('termsOfService','S'),('contact','M:Contact'),('license','M:License'),('version','S'),EXT,('summary','S')], ext='x')
cls('License', [('name','S'),('url','S'),('identifier','S'),EXT], ext='x')
cls('Link', [('operationRef','S'),('operationId','S'),('parameters','MAP:S'),('requestBody','O'),('headers','MAP:M:Header'),('description','S'),('$ref','S'),EXT,('server','M:Server')], ext='x', custom={'$ref'})
cls('LinkParameter', [('value','S'),EXT], ext='x30')
cls('Discriminator', [('propertyName','S'),('mapping','MAP:S'),EXT], ext='disc')
cls('Encoding', [('contentType','S'),('headers','MAP:M:Header'),('style','E:EncodingStyle'),('explode','B'),('allowReserved','B'),EXT], ext='x')
cls('EncodingProperty', [('contentType','S'),('headers','MAP:M:Header'),('style','E:EncodingPropertyStyle'),('explode','B'),('allowReserved','B'),EXT], ext='x')
cls('MediaType', [('schema','M:Schema'),('examples','MAP:M:Example'),('example','O'),('encoding','MAP:M:Encoding'),EXT,('exampleSetFlag','F')], ext='x', custom={'example'}, eq_skip={'exampleSetFlag'})
cls('XML', [('name','S'),('namespace','S'),('prefix','S'),('attribute','B'),('wrapped','B'),EXT], ext='x')
cls('Parameter', [('name','S'),('in','S'),('description','S'),('required','B'),('deprecated','B'),('allowEmptyValue','B'),('$ref','S'),('style','E:ParameterStyle'),('explode','B'),('allowReserved','B'),('schema','M:Schema'),('examples','MAP:M:Example'),('example','O'),('content','M:Content'),EXT], ext='x', custom={'in','$ref','required'}, kind='ParameterKind', custom_new=True)
cls('RequestBody', [('description','S'),('content','M:Content'),('required','B'),EXT,('$ref','S')], ext='x', custom={'$ref'})
cls('ApiResponse', [('description','S'),('headers','MAP:M:Header'),('content','M:Content'),('links','MAP:M:Link'),EXT,('$ref','S')], ext='x', custom={'$ref'})
cls('OAuthFlow', [('authorizationUrl','S'),('tokenUrl','S'),('refreshUrl','S'),('scopes','M:Scopes'),EXT], ext='x')
cls('OAuthFlows', [('implicit','M:OAuthFlow'),('password','M:OAuthFlow'),('clientCredentials','M:OAuthFlow'),('authorizationCode','M:OAuthFlow'),EXT], ext='x')
cls('SecurityScheme', [('type','E:SecuritySchemeType'),('description','S'),('name','S'),('$ref','S'),('in','E:SecuritySchemeIn'),('scheme','S'),('bearerFormat','S'),('flows','M:OAuthFlows'),('openIdConnectUrl','S'),EXT], ext='x', custom={'$ref'})
cls('Server', [('url','S'),('description','S'),('variables','M:ServerVariables'),EXT], ext='x')
cls('ServerVariable', [('enum','L:S'),('default','S'),('description','S'),EXT], ext='x')
cls('Tag', [('name','S'),('description','S'),('externalDocs','M:ExternalDocumentation'),EXT], ext='x')
cls('Schema', [
    ('default','O'),('name','S'),('title','S'),('multipleOf','BD'),('maximum','BD'),('exclusiveMaximum','B'),('minimum','BD'),('exclusiveMinimum','B'),
    ('maxLength','I'),('minLength','I'),('pattern','S'),('maxItems','I'),('minItems','I'),('uniqueItems','B'),('maxProperties','I'),('minProperties','I'),
    ('required','L:S'),('type','S'),('not','M:Schema'),('properties','MAP:M:Schema'),('additionalProperties','AP'),('description','S'),('format','S'),('$ref','S'),
    ('nullable','B'),('readOnly','B'),('writeOnly','B'),('example','O'),('externalDocs','M:ExternalDocumentation'),('deprecated','B'),('xml','M:XML'),EXT,
    ('enum','L:O'),('discriminator','M:Discriminator'),('exampleSetFlag','F'),('defaultSetFlag','F'),('prefixItems','L:M:Schema'),('allOf','L:M:Schema'),
    ('anyOf','L:M:Schema'),('oneOf','L:M:Schema'),('items','M:Schema'),('const','O'),('specVersion','E:SpecVersion'),('types','SET'),('patternProperties','MAP:M:Schema'),
    ('exclusiveMaximumValue','BD'),('exclusiveMinimumValue','BD'),('contains','M:Schema'),('$id','S'),('$schema','S'),('$anchor','S'),('$vocabulary','S'),
    ('$dynamicAnchor','S'),('$dynamicRef','S'),('contentEncoding','S'),('contentMediaType','S'),('contentSchema','M:Schema'),('propertyNames','M:Schema'),
    ('unevaluatedProperties','M:Schema'),('maxContains','I'),('minContains','I'),('additionalItems','M:Schema'),('unevaluatedItems','M:Schema'),('if','M:Schema'),
    ('else','M:Schema'),('then','M:Schema'),('dependentSchemas','MAP:M:Schema'),('dependentRequired','MAP:L:S'),('$comment','S'),('examples','L:O'),
    ('booleanSchemaValue','B'),('jsonSchema','MAP:O'),('jsonSchemaImpl','O')],
    ext='schema', kind='SchemaKind',
    custom={'default','example','const','enum','required','type','$ref','additionalProperties','types','examples'},
    eq_skip={'name','exampleSetFlag','defaultSetFlag','specVersion','jsonSchema','jsonSchemaImpl'}, custom_new=True)

# map-like classes: (name, value type, has_extensions, has_ref)
MAPS = [('Paths','M:PathItem',True,False),('Callback','M:PathItem',True,True),('Content','M:MediaType',False,False),('ApiResponses','M:ApiResponse',True,False),
        ('Scopes','S',True,False),('SecurityRequirement','L:S',False,False),('ServerVariables','M:ServerVariable',True,False)]

KEYWORDS = {'type','in','enum','default','if','else','const','ref','override','use','mod','self','match','loop','move','box','impl','fn','struct','where','static','as','dyn','trait'}
FLUENT_RENAME = {'id': 'id_', 'json_schema': 'json_schema_'}

def snake(name):
    name = name.lstrip('$')
    s = re.sub(r'([A-Z])', r'_\1', name).lower()
    return s

def field_ident(name):
    s = snake(name)
    return s + '_' if s in KEYWORDS else s

def fluent_ident(name):
    s = snake(name)
    if s in FLUENT_RENAME:
        return FLUENT_RENAME[s]
    return s + '_' if s in KEYWORDS else s

def rtype(t):
    if t == 'S': return 'String'
    if t in ('B','F'): return 'bool'
    if t == 'I': return 'i32'
    if t == 'BD': return 'BigDecimal'
    if t == 'O': return 'JavaObject'
    if t == 'SET': return 'JSet'
    if t == 'AP': return 'AdditionalProperties'
    if t.startswith('M:'): return t[2:]
    if t.startswith('E:'): return t[2:]
    if t.startswith('L:'): return 'JList<%s>' % rtype(t[2:])
    if t.startswith('MAP:'): return 'JMap<%s>' % rtype(t[4:])
    raise Exception(t)

def elem_eq(t):
    return True


# ---- Java equals/hashCode (from the decompiled swagger-models sources) ----
DECOMP = os.path.join(ROOT, 'reference/decompiled/swagger-models-2.2.53/io/swagger/v3/oas/models')
_SRC = {}
for dp, _, fs in os.walk(DECOMP):
    for f in fs:
        if f.endswith('.java'):
            _SRC[f[:-5]] = os.path.join(dp, f)

def java_method(cls_name, sig):
    path = _SRC.get(cls_name)
    if not path:
        return None
    src = open(path).read()
    i = src.find(sig)
    if i < 0:
        return None
    j = src.find('\n   }\n', i)
    return src[i:j]

def java_eq_fields(cls_name):
    body = java_method(cls_name, 'public boolean equals(Object o)')
    if body is None:
        return None, False
    fields = []
    for f in re.findall(r'this\.(\$?\w+)', body):
        if f not in fields and f != 'getClass':
            fields.append(f)
    return fields, 'super.equals' in body

def java_hash_items(cls_name):
    body = java_method(cls_name, 'public int hashCode()')
    if body is None:
        return None, None
    if 'Objects.hash' in body:
        inner = re.search(r'Objects\.hash\(new Object\[\]\{(.*)\}\)', body, re.S).group(1)
        items = [x.strip() for x in inner.split(',')]
        return 'objects', ['super' if x == 'super.hashCode()' else x.replace('this.', '') for x in items]
    items = []
    for f in re.findall(r'this\.(\$?\w+)', body):
        if f not in items:
            items.append(f)
    return 'chain', items

# ---- rustdoc text ----
# What each OpenAPI object is (CLASS_DOC) and what each of its properties holds (FIELD_DOC, keyed by property name,
# or by `Class.property` where a class gives the name another meaning). Every generated public item is documented
# from these, so `missing_docs` holds for the generated code too.
CLASS_DOC = {
    'OpenAPI': 'the root of an OpenAPI 3.x document',
    'Components': 'the reusable objects a document declares under `components`',
    'ExternalDocumentation': 'a link to documentation kept outside the document',
    'Operation': 'one HTTP operation on a path',
    'PathItem': 'the operations, servers and parameters of one path',
    'Example': 'an example value, inline or by `externalValue`',
    'Header': 'a response or encoding header',
    'Contact': 'the contact information of the API',
    'Info': 'the API metadata: title, version, description, contact and licence',
    'License': 'the licence the API is published under',
    'Link': 'a design-time link from a response to another operation',
    'LinkParameter': 'a parameter value passed along a link',
    'Discriminator': 'the property that selects one of several schemas',
    'Encoding': 'how one property of a request body part is serialized',
    'EncodingProperty': 'the encoding of one property (swagger-core\'s alternative `Encoding` class)',
    'MediaType': 'the schema and examples of one media type',
    'XML': 'the XML representation of a schema',
    'Parameter': 'an operation parameter (query, path, header or cookie)',
    'RequestBody': 'the body an operation accepts',
    'ApiResponse': 'one response of an operation',
    'OAuthFlow': 'the endpoints and scopes of one OAuth 2.0 flow',
    'OAuthFlows': 'the OAuth 2.0 flows a security scheme supports',
    'SecurityScheme': 'a security scheme operations can require',
    'Server': 'a server the API is reached at',
    'ServerVariable': 'a variable substituted into a server URL template',
    'Tag': 'a tag that groups operations',
    'Schema': 'a JSON Schema (OpenAPI 3.0 dialect or 3.1 / 2020-12), every `Schema` subclass in one type',
    'Paths': 'the relative paths of the API and their path items, in document order',
    'Callback': 'the path items of one callback, keyed by runtime expression',
    'Content': 'the media types of a body or parameter, keyed by media type name',
    'ApiResponses': 'the responses of an operation, keyed by status code or `default`',
    'Scopes': 'the scopes of an OAuth 2.0 flow, scope name to description',
    'SecurityRequirement': 'a security requirement, scheme name to required scopes',
    'ServerVariables': 'the variables of a server URL template, keyed by name',
}
FIELD_DOC = {
    '$anchor': 'the `$anchor` keyword (JSON Schema 2020-12)',
    '$comment': 'the `$comment` keyword (JSON Schema 2020-12)',
    '$dynamicAnchor': 'the `$dynamicAnchor` keyword (JSON Schema 2020-12)',
    '$dynamicRef': 'the `$dynamicRef` keyword (JSON Schema 2020-12)',
    '$id': 'the `$id` keyword (JSON Schema 2020-12)',
    '$ref': 'the `$ref` this object stands for, as written or as normalized by the setter',
    '$schema': 'the `$schema` keyword (JSON Schema 2020-12)',
    '$vocabulary': 'the `$vocabulary` keyword (JSON Schema 2020-12)',
    'additionalItems': 'the `additionalItems` schema',
    'additionalProperties': 'the `additionalProperties` value: a boolean or a schema',
    'allOf': 'the `allOf` schemas (a composed schema)',
    'allowEmptyValue': 'whether an empty value may be sent',
    'allowReserved': 'whether RFC 3986 reserved characters pass unencoded',
    'anyOf': 'the `anyOf` schemas (a composed schema)',
    'attribute': 'whether the property is an XML attribute rather than an element',
    'authorizationCode': 'the authorization code flow',
    'authorizationUrl': 'the authorization URL of the flow',
    'bearerFormat': 'a hint at how the bearer token is formatted',
    'booleanSchemaValue': 'the value of a boolean schema (`true` / `false` in place of an object, OpenAPI 3.1)',
    'callbacks': 'the callbacks, keyed by name',
    'clientCredentials': 'the client credentials flow',
    'components': 'the reusable components',
    'const': 'the `const` value',
    'contact': 'the contact information',
    'contains': 'the `contains` schema',
    'content': 'the media types and their schemas',
    'contentEncoding': 'the `contentEncoding` keyword',
    'contentMediaType': 'the `contentMediaType` keyword',
    'contentSchema': 'the `contentSchema` keyword',
    'contentType': 'the content type(s) of the property, comma-separated',
    'default': 'the `default` value',
    'defaultSetFlag': 'whether `default` was set, even to `null`',
    'delete': 'the DELETE operation',
    'dependentRequired': 'the `dependentRequired` keyword: property name to the properties it requires',
    'dependentSchemas': 'the `dependentSchemas` keyword: property name to schema',
    'deprecated': 'whether it is deprecated',
    'description': 'the description (CommonMark)',
    'discriminator': 'the discriminator for polymorphism',
    'else': 'the `else` schema',
    'email': 'the email address',
    'encoding': 'the encoding of each property, keyed by property name',
    'enum': 'the allowed values',
    'example': 'the example value',
    'exampleSetFlag': 'whether `example` was set, even to `null`',
    'examples': 'the examples, keyed by name',
    'Schema.examples': 'the `examples` keyword (OpenAPI 3.1)',
    'exclusiveMaximum': 'whether `maximum` is exclusive (OpenAPI 3.0)',
    'exclusiveMaximumValue': 'the numeric `exclusiveMaximum` (OpenAPI 3.1)',
    'exclusiveMinimum': 'whether `minimum` is exclusive (OpenAPI 3.0)',
    'exclusiveMinimumValue': 'the numeric `exclusiveMinimum` (OpenAPI 3.1)',
    'explode': 'whether array and object values become separate parameters',
    'externalDocs': 'the external documentation',
    'externalValue': 'the URL of an example kept outside the document',
    'extensions': 'the `x-` vendor extensions, in document order',
    'flows': 'the OAuth 2.0 flows',
    'format': 'the `format` refining `type` (`int64`, `date-time`, ...)',
    'get': 'the GET operation',
    'head': 'the HEAD operation',
    'headers': 'the headers, keyed by name',
    'identifier': 'the SPDX licence expression (OpenAPI 3.1)',
    'if': 'the `if` schema',
    'implicit': 'the implicit flow',
    'in': 'where the value is sent: `query`, `header`, `path` or `cookie`',
    'info': 'the API metadata',
    'items': 'the schema of array items',
    'jsonSchema': 'the raw JSON Schema object a 3.1 schema was read from',
    'jsonSchemaDialect': 'the default `$schema` of the document\'s schemas (OpenAPI 3.1)',
    'jsonSchemaImpl': 'the implementation object swagger-core attaches to a 3.1 schema',
    'license': 'the licence',
    'links': 'the links, keyed by name',
    'mapping': 'the discriminator values and the schema `$ref` each selects',
    'maxContains': 'the `maxContains` keyword',
    'maxItems': 'the maximum number of array items',
    'maxLength': 'the maximum string length',
    'maxProperties': 'the maximum number of object properties',
    'maximum': 'the maximum value',
    'minContains': 'the `minContains` keyword',
    'minItems': 'the minimum number of array items',
    'minLength': 'the minimum string length',
    'minProperties': 'the minimum number of object properties',
    'minimum': 'the minimum value',
    'multipleOf': 'the number the value must be a multiple of',
    'name': 'the name',
    'Schema.name': 'the name the schema was declared under (not serialized)',
    'namespace': 'the XML namespace URI',
    'not': 'the `not` schema',
    'nullable': 'whether `null` is allowed (OpenAPI 3.0)',
    'oneOf': 'the `oneOf` schemas (a composed schema)',
    'openIdConnectUrl': 'the OpenID Connect discovery URL',
    'openapi': 'the OpenAPI version the document declares (`3.0.1`, `3.1.0`, ...)',
    'operationId': 'the operation id',
    'Link.operationId': 'the id of the linked operation',
    'operationRef': 'a reference to the linked operation',
    'options': 'the OPTIONS operation',
    'parameters': 'the parameters',
    'Components.parameters': 'the reusable parameters, keyed by name',
    'Link.parameters': 'the values passed to the linked operation\'s parameters, keyed by parameter name',
    'password': 'the resource owner password flow',
    'patch': 'the PATCH operation',
    'pathItems': 'the reusable path items, keyed by name (OpenAPI 3.1)',
    'paths': 'the paths and their operations',
    'pattern': 'the regular expression a string must match',
    'patternProperties': 'the `patternProperties` keyword: regular expression to schema',
    'post': 'the POST operation',
    'prefix': 'the XML namespace prefix',
    'prefixItems': 'the `prefixItems` schemas',
    'properties': 'the object properties and their schemas, in document order',
    'propertyName': 'the name of the property holding the discriminator value',
    'propertyNames': 'the `propertyNames` schema',
    'put': 'the PUT operation',
    'readOnly': 'whether the property is only sent in responses',
    'refreshUrl': 'the URL for refreshing tokens',
    'requestBodies': 'the reusable request bodies, keyed by name',
    'requestBody': 'the request body',
    'Link.requestBody': 'the value or expression sent as the linked operation\'s request body',
    'required': 'whether it is required',
    'Schema.required': 'the names of the required properties',
    'responses': 'the responses',
    'Components.responses': 'the reusable responses, keyed by name',
    'schema': 'the schema of the value',
    'schemas': 'the reusable schemas, keyed by name',
    'scheme': 'the HTTP authorization scheme (`basic`, `bearer`, ...)',
    'scopes': 'the available scopes and their descriptions',
    'security': 'the security requirements, any one of which suffices',
    'securitySchemes': 'the security schemes, keyed by name',
    'server': 'the server the linked operation is called on',
    'servers': 'the servers',
    'specVersion': 'which OpenAPI version (3.0 or 3.1) the object follows',
    'style': 'how the value is serialized',
    'summary': 'the short summary',
    'tags': 'the tags',
    'Operation.tags': 'the names of the tags the operation is grouped under',
    'termsOfService': 'the URL of the terms of service',
    'then': 'the `then` schema',
    'title': 'the title',
    'tokenUrl': 'the token URL of the flow',
    'trace': 'the TRACE operation',
    'type': 'the type',
    'Schema.type': 'the `type` (OpenAPI 3.0: a single name)',
    'SecurityScheme.type': 'the scheme type: `apiKey`, `http`, `oauth2`, `openIdConnect` or `mutualTLS`',
    'types': 'the `type` names (OpenAPI 3.1: one or several)',
    'unevaluatedItems': 'the `unevaluatedItems` schema',
    'unevaluatedProperties': 'the `unevaluatedProperties` schema',
    'uniqueItems': 'whether array items must be unique',
    'url': 'the URL',
    'Server.url': 'the server URL, possibly a template with `{variables}`',
    'value': 'the value',
    'Example.value': 'the example value',
    'valueSetFlag': 'whether `value` was set, even to `null`',
    'variables': 'the variables substituted into the URL template',
    'version': 'the API version (not the OpenAPI version)',
    'webhooks': 'the webhooks, keyed by name (OpenAPI 3.1)',
    'wrapped': 'whether an array is wrapped in an outer element',
    'writeOnly': 'whether the property is only sent in requests',
    'xml': 'the XML representation',
}
KIND_DOC = {
    'SchemaKind': 'which `Schema` subclass this is (`StringSchema`, `ArraySchema`, ...), as Java\'s runtime class',
    'ParameterKind': 'which `Parameter` subclass this is (`QueryParameter`, `PathParameter`, ...), as Java\'s runtime class',
}

def java_class(name):
    """The fully qualified swagger-models class name."""
    path = _SRC.get(name)
    if not path:
        return name
    return 'io.swagger.v3.oas.models.' + os.path.relpath(path, DECOMP)[:-5].replace(os.sep, '.')

def field_doc(cls_name, f):
    d = FIELD_DOC.get('%s.%s' % (cls_name, f)) or FIELD_DOC.get(f)
    if d is None:
        raise Exception('no FIELD_DOC for %s.%s' % (cls_name, f))
    return d

def java_prop(f):
    """The Java bean property name of a spec field (`$ref` → `ref`)."""
    return f.lstrip('$')

def java_cap(f):
    p = java_prop(f)
    return p[0].upper() + p[1:]

def java_to_spec_field(jf, fields):
    names = [f for f, _ in fields]
    if jf in names:
        return jf
    if jf.startswith('_') and jf[1:] in names:
        return jf[1:]
    raise Exception('unmapped java field %s' % jf)

def hash_expr(t, x):
    return '%s.java_hash_code()' % x

out = []
w = out.append
w('// @generated by tools/gen_models.py — do not edit by hand.')
w('#![allow(clippy::all, unused_imports, unused_variables, dead_code)]')
w('use super::*;')
w('use crate::java::{IntoNullable, JList, JMap, JSet};')
w('use jackson::{BigDecimal, JacksonError, JavaObject, JsonGenerator, JsonNode};')
w('use parking_lot::RwLock;')
w('use std::sync::Arc;')
w('')

def gen_class(name, spec):
    fields = spec['fields']
    kind = spec.get('kind')
    custom = spec.get('custom', set())
    eq_skip = spec.get('eq_skip', set())
    jc = java_class(name)
    w('/// Backing fields of [`%s`], one per property of `%s`; `None` is Java\'s `null`.' % (name, jc))
    w('#[derive(Debug, Default)]')
    w('pub struct %sData {' % name)
    if kind:
        w('    /// The %s.' % KIND_DOC[kind])
        w('    pub kind: %s,' % kind)
    for f, t in fields:
        w('    /// `%s`: %s.' % (java_prop(f), field_doc(name, f)))
        if t == 'F':
            w('    pub %s: bool,' % field_ident(f))
        else:
            w('    pub %s: Option<%s>,' % (field_ident(f), rtype(t)))
    w('}')
    w('')
    w('/// `%s`: %s.' % (jc, CLASS_DOC[name]))
    w('///')
    w('/// A shared, mutable handle with Java reference semantics: a clone is the same object, and a change made')
    w('/// through one handle shows through every other.')
    w('#[derive(Clone, Debug)]')
    w('pub struct %s(pub Arc<RwLock<%sData>>);' % (name, name))
    w('')
    w('impl %s {' % name)
    w('    /// A new object holding `d`.')
    w('    pub fn from_data(d: %sData) -> %s { %s(Arc::new(RwLock::new(d))) }' % (name, name, name))
    if not spec.get('custom_new'):
        w('    /// `new %s()`: every property unset.' % name)
        w('    pub fn new() -> %s { %s::from_data(%sData::default()) }' % (name, name, name))
    w('    /// Whether both handles are the same object (Java\'s `==`).')
    w('    pub fn ptr_eq(&self, other: &%s) -> bool { Arc::ptr_eq(&self.0, &other.0) }' % name)
    w('    /// Identity (for `IdentityHashMap`-style bookkeeping).')
    w('    pub fn id(&self) -> usize { Arc::as_ptr(&self.0) as *const () as usize }')
    w('    /// Locks the fields for reading.')
    w('    pub fn read(&self) -> parking_lot::RwLockReadGuard<\'_, %sData> { self.0.read() }' % name)
    w('    /// Locks the fields for writing.')
    w('    pub fn write(&self) -> parking_lot::RwLockWriteGuard<\'_, %sData> { self.0.write() }' % name)
    for f, t in fields:
        fi = field_ident(f)
        getter = 'get_' + snake(f)
        setter = 'set_' + snake(f)
        fluent = fluent_ident(f)
        rt = rtype(t)
        if f in custom:
            continue
        fd = field_doc(name, f)
        jg, js = 'get' + java_cap(f), 'set' + java_cap(f)
        if t == 'F':
            w('    /// `%s()`: %s.' % (jg, fd))
            w('    pub fn %s(&self) -> bool { self.0.read().%s }' % (getter, fi))
            w('    /// `%s(v)`: records %s.' % (js, fd))
            w('    pub fn %s(&self, v: bool) { self.0.write().%s = v; }' % (setter, fi))
            continue
        w('    /// `%s()`: %s, `None` when unset.' % (jg, fd))
        w('    pub fn %s(&self) -> Option<%s> { self.0.read().%s.clone() }' % (getter, rt, fi))
        if f == 'extensions':
            continue
        w('    /// `%s(v)`: sets %s; `None` unsets it.' % (js, fd))
        w('    pub fn %s(&self, v: impl IntoNullable<%s>) { self.0.write().%s = v.into_nullable(); }' % (setter, rt, fi))
        w('    /// `%s(v)`: sets %s and returns this handle, for chaining.' % (java_prop(f), fd))
        w('    pub fn %s(&self, v: impl IntoNullable<%s>) -> %s { self.%s(v); self.clone() }' % (fluent, rt, name, setter))
    w('}')
    w('')
    # equality: the field set of the Java equals()
    jeq, _ = java_eq_fields(name)
    w('impl PartialEq for %s {' % name)
    w('    fn eq(&self, other: &Self) -> bool {')
    w('        if self.ptr_eq(other) { return true; }')
    w('        let (a, b) = (self.0.read(), other.0.read());')
    conds = []
    if kind:
        conds.append('a.kind == b.kind')
    for jf in jeq:
        f = java_to_spec_field(jf, fields)
        conds.append('a.%s == b.%s' % (field_ident(f), field_ident(f)))
    w('        ' + ' && '.join(conds or ['true']))
    w('    }')
    w('}')
    w('')
    # hashCode: the Java hashCode() of the class (Schema/Parameter subclasses wrap the base value)
    style, items = java_hash_items(name)
    types = dict(fields)
    w('impl JavaHash for %s {' % name)
    w('    fn java_hash_code(&self) -> i32 {')
    w('        let d = self.0.read();')
    parts = []
    for it in items:
        f = java_to_spec_field(it, fields)
        parts.append('d.%s.java_hash_code()' % field_ident(f))
    fn = 'objects_hash' if style == 'objects' else 'chain_hash'
    w('        let base = %s(&[%s]);' % (fn, ', '.join(parts)))
    if name == 'Schema':
        w('        match d.kind {')
        w('            SchemaKind::Array | SchemaKind::Composed | SchemaKind::Json | SchemaKind::Schema => base,')
        w('            SchemaKind::Binary => objects_hash(&[d.default_.java_hash_code(), d.enum_.java_hash_code(), base]),')
        w('            _ => objects_hash(&[base]),')
        w('        }')
    elif name == 'Parameter':
        w('        match d.kind {')
        w('            ParameterKind::Parameter => base,')
        w('            ParameterKind::Path => objects_hash(&[d.in_.java_hash_code(), d.required.java_hash_code(), base]),')
        w('            _ => objects_hash(&[d.in_.java_hash_code(), base]),')
        w('        }')
    else:
        w('        base')
    w('    }')
    w('}')
    w('')

for name, spec in CLASSES.items():
    gen_class(name, spec)

# serialization
def ser_value(t, expr, mapper='m'):
    """Rust expression writing value `expr` (a reference) of type t."""
    if t == 'S': return 'g.write_string(%s)?;' % expr
    if t in ('B','F'): return 'g.write_boolean(*%s)?;' % expr
    if t == 'I': return 'g.write_number_i32(*%s)?;' % expr
    if t == 'BD': return 'g.write_number_big_decimal(%s)?;' % expr
    if t == 'O': return 'write_object(g, %s, %s)?;' % (expr, mapper)
    if t == 'SET': return 'g.write_start_array()?; for s in %s.items() { g.write_string(&s)?; } g.write_end_array()?;' % expr
    if t == 'AP': return '%s.serialize(g, %s)?;' % (expr, mapper)
    if t.startswith('M:'): return '%s.serialize(g, %s)?;' % (expr, mapper)
    if t.startswith('E:'): return 'g.write_string(%s.as_str())?;' % expr
    if t.startswith('L:'):
        inner = t[2:]
        return 'g.write_start_array()?; for e in %s.to_vec().iter() { %s } g.write_end_array()?;' % (expr, ser_value(inner, 'e', mapper))
    if t.startswith('MAP:'):
        inner = t[4:]
        return 'write_map(g, %s, %s, |g, e| { %s Ok(()) })?;' % (expr, mapper, ser_value(inner, 'e', mapper))
    raise Exception(t)

JACKSON_NAME = {}
def prop_to_field(name, spec):
    # json property name -> (field name, type)
    for f, t in spec['fields']:
        if f == name:
            return f, t
    alias = {'exclusiveMaximum31':'exclusiveMaximumValue'}
    return None, None

w('/// Which configured `ObjectMapper` is serializing (property sets and orders differ).')
for name, spec in CLASSES.items():
    w('impl %s {' % name)
    w('    /// Bean properties in Jackson order (the "unwrapping" serializer: no start/end object).')
    w('    pub fn serialize_fields(&self, g: &mut dyn JsonGenerator, m: Mapper) -> Result<(), JacksonError> {')
    w('        let d = self.0.read();')
    w('        match m {')
    for mapper, rust_mapper in (('json','Mapper::Json30'),('json31','Mapper::Json31'),('converter','Mapper::Converter')):
        key = (mapper, name)
        props = ORDER.get(key)
        if props is None:
            raise Exception('no order for %s' % (key,))
        w('            %s => {' % rust_mapper)
        for (pname, anyget, suppress) in props:
            if anyget:
                w('                if let Some(x) = &d.extensions { write_any_getter(g, x, m, %s)?; }' % ('true' if suppress else 'false'))
                continue
            # special 3.1 schema mappings
            fname, ftype = pname, None
            if name == 'Schema' and mapper == 'json31' and pname == 'type':
                w('                if let Some(x) = &d.types { write_types(g, x)?; }')
                continue
            if name == 'Schema' and mapper == 'json31' and pname == 'exclusiveMaximum':
                w('                if let Some(x) = &d.exclusive_maximum_value { g.write_field_name("exclusiveMaximum")?; g.write_number_big_decimal(x)?; }')
                continue
            if name == 'Schema' and mapper == 'json31' and pname == 'exclusiveMinimum':
                w('                if let Some(x) = &d.exclusive_minimum_value { g.write_field_name("exclusiveMinimum")?; g.write_number_big_decimal(x)?; }')
                continue
            f, t = prop_to_field(pname, spec)
            if f is None:
                raise Exception('unknown property %s.%s' % (name, pname))
            fi = field_ident(f)
            if t == 'F':
                w('                g.write_field_name("%s")?; g.write_boolean(d.%s)?;' % (pname, fi))
                continue
            special = spec.get('special_ser', {}).get(pname)
            # per-field hooks for custom (de)serializers referenced from mixins
            if (name, pname) in (('OpenAPI','paths'),):
                w('                if let Some(x) = &d.%s { g.write_field_name("%s")?; x.serialize_with_extensions(g, m)?; }' % (fi, pname))
                continue
            if (name, pname) in (('Operation','responses'),):
                w('                if let Some(x) = &d.%s { g.write_field_name("%s")?; x.serialize_with_extensions(g, m)?; }' % (fi, pname))
                continue
            if (name, pname) in (('Operation','callbacks'),('Components','callbacks')):
                w('                if let Some(x) = &d.%s { g.write_field_name("%s")?; write_map(g, x, m, |g, e| { e.serialize_callback(g, m)?; Ok(()) })?; }' % (fi, pname))
                continue
            if name == 'Schema' and pname in ('example','default','enum') and True:
                w('                if let Some(x) = &d.%s { g.write_field_name("%s")?; self.write_typed_value(g, x, m, "%s")?; }' % (fi, pname, pname) if pname != 'enum' else
                  '                if let Some(x) = &d.%s { g.write_field_name("%s")?; g.write_start_array()?; for e in x.to_vec().iter() { self.write_typed_value(g, e, m, "enum")?; } g.write_end_array()?; }' % (fi, pname))
                continue
            if name == 'Schema' and pname in ('const','examples'):
                if pname == 'const':
                    w('                if let Some(x) = &d.const_ { g.write_field_name("const")?; self.write_typed_value(g, x, m, "const")?; }')
                else:
                    w('                if let Some(x) = &d.examples { g.write_field_name("examples")?; g.write_start_array()?; for e in x.to_vec().iter() { self.write_typed_value(g, e, m, "examples")?; } g.write_end_array()?; }')
                continue
            if (name, pname) in (('MediaType','example'), ('Example','value')):
                w('                if let Some(x) = &d.%s { g.write_field_name("%s")?; write_object_content_always(g, x, m)?; }' % (fi, pname))
                continue
            w('                if let Some(x) = &d.%s { g.write_field_name("%s")?; %s }' % (fi, pname, ser_value(t, 'x')))
        w('            }')
    w('        }')
    w('        Ok(())')
    w('    }')
    w('}')
    w('')

# map-like classes
for (name, vt, has_ext, has_ref) in MAPS:
    rt = rtype(vt)
    jc = java_class(name)
    w('/// Backing fields of [`%s`]: its entries and the properties `%s` adds to the map.' % (name, jc))
    w('#[derive(Debug, Default)]')
    w('pub struct %sData {' % name)
    w('    /// The entries, in insertion order (Java\'s `LinkedHashMap`).')
    w('    pub map: indexmap::IndexMap<String, %s>,' % rt)
    if has_ext:
        w('    /// `extensions`: %s.' % FIELD_DOC['extensions'])
        w('    pub extensions: Option<JMap<JavaObject>>,')
    if has_ref:
        w('    /// `$ref`: %s.' % FIELD_DOC['$ref'])
        w('    pub ref_: Option<String>,')
    w('}')
    w('')
    w('/// `%s extends LinkedHashMap<String, %s>`: %s.' % (jc, rt, CLASS_DOC[name]))
    w('///')
    w('/// A shared, mutable handle with Java reference semantics: a clone is the same map.')
    w('#[derive(Clone, Debug)]')
    w('pub struct %s(pub Arc<RwLock<%sData>>);' % (name, name))
    w('')
    w('impl Default for %s { fn default() -> Self { %s::new() } }' % (name, name))
    w('impl %s {' % name)
    w('    /// `new %s()`: an empty map.' % name)
    w('    pub fn new() -> %s { %s(Arc::new(RwLock::new(%sData::default()))) }' % (name, name, name))
    w('    /// Whether both handles are the same map (Java\'s `==`).')
    w('    pub fn ptr_eq(&self, other: &%s) -> bool { Arc::ptr_eq(&self.0, &other.0) }' % name)
    w('    /// Identity (for `IdentityHashMap`-style bookkeeping).')
    w('    pub fn id(&self) -> usize { Arc::as_ptr(&self.0) as *const () as usize }')
    w('    /// `get(k)`: the value under `k`, `None` when absent.')
    w('    pub fn get(&self, k: &str) -> Option<%s> { self.0.read().map.get(k).cloned() }' % rt)
    w('    /// `put(k, v)`: stores `v` under `k`, keeping an existing key\'s position; returns the value it replaced.')
    w('    pub fn put(&self, k: impl Into<String>, v: %s) -> Option<%s> { let k = k.into(); let mut d = self.0.write(); if let Some(s) = d.map.get_mut(&k) { return Some(std::mem::replace(s, v)); } d.map.insert(k, v); None }' % (rt, rt))
    w('    /// `remove(k)`: takes the entry out, keeping the others in order; returns its value.')
    w('    pub fn remove(&self, k: &str) -> Option<%s> { self.0.write().map.shift_remove(k) }' % rt)
    w('    /// `containsKey(k)`.')
    w('    pub fn contains_key(&self, k: &str) -> bool { self.0.read().map.contains_key(k) }')
    w('    /// The number of entries.')
    w('    pub fn len(&self) -> usize { self.0.read().map.len() }')
    w('    /// `size()`: the number of entries.')
    w('    pub fn size(&self) -> usize { self.len() }')
    w('    /// `isEmpty()`.')
    w('    pub fn is_empty(&self) -> bool { self.0.read().map.is_empty() }')
    w('    /// `clear()`: removes every entry.')
    w('    pub fn clear(&self) { self.0.write().map.clear() }')
    w('    /// `keySet()`: the keys, in order.')
    w('    pub fn keys(&self) -> Vec<String> { self.0.read().map.keys().cloned().collect() }')
    w('    /// `values()`: the values, in order.')
    w('    pub fn values(&self) -> Vec<%s> { self.0.read().map.values().cloned().collect() }' % rt)
    w('    /// `entrySet()`: the entries, in order.')
    w('    pub fn entries(&self) -> Vec<(String, %s)> { self.0.read().map.iter().map(|(k, v)| (k.clone(), v.clone())).collect() }' % rt)
    w('    /// `putAll(other)`: puts each entry of `other`, in its order.')
    w('    pub fn put_all(&self, other: &%s) { for (k, v) in other.entries() { self.put(k, v); } }' % name)
    if has_ext:
        w('    /// `getExtensions()`: %s, `None` when unset.' % FIELD_DOC['extensions'])
        w('    pub fn get_extensions(&self) -> Option<JMap<JavaObject>> { self.0.read().extensions.clone() }')
    if has_ref:
        w('    /// `get$ref()`: %s, `None` when unset.' % FIELD_DOC['$ref'])
        w('    pub fn get_ref(&self) -> Option<String> { self.0.read().ref_.clone() }')
    w('}')
    w('')
    w('impl PartialEq for %s {' % name)
    w('    fn eq(&self, other: &Self) -> bool {')
    w('        if self.ptr_eq(other) { return true; }')
    w('        let (a, b) = (self.0.read(), other.0.read());')
    c = ['a.map.len() == b.map.len() && a.map.iter().all(|(k, v)| b.map.get(k).is_some_and(|w| v == w))']
    jeq, _ = java_eq_fields(name)
    for jf in jeq or []:
        c.append('a.%s == b.%s' % (field_ident(jf), field_ident(jf)))
    w('        ' + ' && '.join(c))
    w('    }')
    w('}')
    w('')
    style, items = java_hash_items(name)
    parts = []
    for it in items:
        if it == 'super':
            parts.append('d.map.iter().fold(0i32, |acc, (k, v)| acc.wrapping_add(java_compat::lang::string_hash(k) ^ v.java_hash_code()))')
        else:
            parts.append('d.%s.java_hash_code()' % field_ident(it))
    w('impl JavaHash for %s {' % name)
    w('    fn java_hash_code(&self) -> i32 {')
    w('        let d = self.0.read();')
    w('        objects_hash(&[%s])' % ', '.join(parts))
    w('    }')
    w('}')
    w('')
    w('impl %s {' % name)
    w('    /// Default Jackson `MapSerializer` output (entries only, null values skipped).')
    w('    pub fn serialize(&self, g: &mut dyn JsonGenerator, m: Mapper) -> Result<(), JacksonError> {')
    w('        g.write_start_object()?;')
    w('        for (k, e) in ordered_entries(self.entries()) { g.write_field_name(&k)?; let e = &e; %s }' % ser_value(vt, 'e'))
    w('        g.write_end_object()')
    w('    }')
    w('}')
    w('')

open(os.path.join(ROOT, 'src/models/generated.rs'), 'w').write('\n'.join(out) + '\n')
print('ok', len(out), 'lines')

# ---------------- bean deserialization ----------------
out2 = []
w2 = out2.append
w2('// @generated by tools/gen_models.py — do not edit by hand.')
w2('#![allow(clippy::all, unused_imports, unused_variables, dead_code, unused_mut)]')
w2('use super::*;')
w2('use super::deserialize::*;')
w2('use crate::java::{JList, JMap, JSet};')
w2('use jackson::{JacksonError, JavaObject, JsonNode};')
w2('')

def de_value(t, expr, cls_name):
    """Rust expression converting `expr: &JsonNode` into Option<T> (Result)."""
    if t == 'S': return 'de_string(%s)?' % expr
    if t in ('B',): return 'de_bool(%s)?' % expr
    if t == 'F': return 'de_bool(%s)?.unwrap_or(false)' % expr
    if t == 'I': return 'de_int(%s)?' % expr
    if t == 'BD': return 'de_big_decimal(%s)?' % expr
    if t == 'O': return 'de_object(%s)' % expr
    if t == 'SET': return 'de_string_set(%s)?' % expr
    if t == 'AP': return 'de_additional_properties(%s, m)?' % expr
    if t.startswith('M:'): return 'de_model::<%s>(%s, m)?' % (t[2:], expr)
    if t.startswith('E:'): return 'de_enum(%s, %s::from_str_value, "%s")?' % (expr, t[2:], t[2:])
    if t.startswith('L:'):
        inner = t[2:]
        return 'de_list(%s, |e| Ok(%s))?' % (expr, de_value(inner, 'e', cls_name))
    if t.startswith('MAP:'):
        inner = t[4:]
        return 'de_map(%s, |e| Ok(%s))?' % (expr, de_value(inner, 'e', cls_name))
    raise Exception(t)

CUSTOM_DESER = {'Schema', 'Parameter', 'SecurityScheme'}
for name, spec in CLASSES.items():
    fields = spec['fields']
    w2('impl %s {' % name)
    w2('    /// Jackson bean deserialization: each JSON field (in document order) goes to its setter.')
    w2('    pub fn deserialize_fields(&self, obj: &JsonNode, m: Mapper) -> Result<(), JacksonError> {')
    w2('        for (k, v) in obj.as_object().into_iter().flat_map(|o| o.iter()) {')
    w2('            match (m, k.as_str()) {')
    # Whether every mapper ends in its own catch-all (an any-setter), which leaves nothing for `_` to match.
    every_mapper_catches_all = True
    for mapper, rust_mapper in (('json','Mapper::Json30'),('json31','Mapper::Json31'),('converter','Mapper::Converter')):
        props = ORDER.get((mapper, name)) or []
        for (pname, anyget, suppress) in props:
            if anyget:
                continue
            if name == 'Schema' and mapper == 'json31' and pname in ('type',):
                w2('                (%s, "type") => { self.set_types(de_types31(v)?); }' % rust_mapper)
                continue
            if name == 'Schema' and mapper == 'json31' and pname in ('exclusiveMaximum', 'exclusiveMinimum'):
                fld = 'exclusive_maximum_value' if pname == 'exclusiveMaximum' else 'exclusive_minimum_value'
                w2('                (%s, "%s") => { self.set_%s(de_big_decimal(v)?); }' % (rust_mapper, pname, fld))
                continue
            f, t = prop_to_field(pname, spec)
            if f is None:
                continue
            setter = 'set_' + snake(f)
            if name == 'Schema' and pname in ('enum', 'examples'):
                w2('                (%s, "%s") => { let kind = self.kind(); self.%s(de_list(v, |e| de_schema_typed(e, kind))?); }' % (rust_mapper, pname, setter))
                continue
            if t == 'F':
                w2('                (%s, "%s") => { self.%s(%s); }' % (rust_mapper, pname, setter, de_value(t, 'v', name)))
            else:
                w2('                (%s, "%s") => { self.%s(%s); }' % (rust_mapper, pname, setter, de_value(t, 'v', name)))
        anyg = [p for p in props if p[1]]
        if anyg:
            w2('                (%s, _) => { self.add_extension(k, de_object(v).unwrap_or(JavaObject::Null)); }' % rust_mapper)
        else:
            every_mapper_catches_all = False
    if not every_mapper_catches_all:
        w2('                _ => {}')
    w2('            }')
    w2('        }')
    w2('        Ok(())')
    w2('    }')
    if name not in CUSTOM_DESER:
        w2('')
        w2('    /// `mapper.convertValue(node, %s.class)`' % name)
        w2('    pub fn from_tree(node: &JsonNode, m: Mapper) -> Result<%s, JacksonError> {' % name)
        w2('        expect_object(node, "%s")?;' % name)
        w2('        let x = %s::new();' % name)
        w2('        x.deserialize_fields(node, m)?;')
        w2('        Ok(x)')
        w2('    }')
    w2('}')
    w2('')
    if name not in CUSTOM_DESER:
        w2('impl FromTree for %s {' % name)
        w2('    fn from_tree(node: &JsonNode, m: Mapper) -> Result<Self, JacksonError> { %s::from_tree(node, m) }' % name)
        w2('}')
        w2('')

open(os.path.join(ROOT, 'src/models/generated_de.rs'), 'w').write('\n'.join(out2) + '\n')
print('ok de', len(out2), 'lines')
