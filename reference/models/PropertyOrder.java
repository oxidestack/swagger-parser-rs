import com.fasterxml.jackson.databind.*;
import com.fasterxml.jackson.databind.ser.*;
import com.fasterxml.jackson.databind.ser.std.*;
import io.swagger.v3.core.util.*;
import java.lang.reflect.*;
import java.util.*;

public class PropertyOrder {
    static String[] CLASSES = {
        "io.swagger.v3.oas.models.OpenAPI","io.swagger.v3.oas.models.Components","io.swagger.v3.oas.models.ExternalDocumentation",
        "io.swagger.v3.oas.models.Operation","io.swagger.v3.oas.models.PathItem","io.swagger.v3.oas.models.Paths",
        "io.swagger.v3.oas.models.callbacks.Callback","io.swagger.v3.oas.models.examples.Example","io.swagger.v3.oas.models.headers.Header",
        "io.swagger.v3.oas.models.info.Contact","io.swagger.v3.oas.models.info.Info","io.swagger.v3.oas.models.info.License",
        "io.swagger.v3.oas.models.links.Link","io.swagger.v3.oas.models.links.LinkParameter",
        "io.swagger.v3.oas.models.media.Schema","io.swagger.v3.oas.models.media.ArraySchema","io.swagger.v3.oas.models.media.ComposedSchema",
        "io.swagger.v3.oas.models.media.DateSchema","io.swagger.v3.oas.models.media.JsonSchema","io.swagger.v3.oas.models.media.MapSchema",
        "io.swagger.v3.oas.models.media.ObjectSchema","io.swagger.v3.oas.models.media.StringSchema","io.swagger.v3.oas.models.media.IntegerSchema",
        "io.swagger.v3.oas.models.media.BooleanSchema","io.swagger.v3.oas.models.media.ArbitrarySchema",
        "io.swagger.v3.oas.models.media.Content","io.swagger.v3.oas.models.media.Discriminator","io.swagger.v3.oas.models.media.Encoding",
        "io.swagger.v3.oas.models.media.EncodingProperty","io.swagger.v3.oas.models.media.MediaType","io.swagger.v3.oas.models.media.XML",
        "io.swagger.v3.oas.models.parameters.Parameter","io.swagger.v3.oas.models.parameters.QueryParameter","io.swagger.v3.oas.models.parameters.RequestBody",
        "io.swagger.v3.oas.models.responses.ApiResponse","io.swagger.v3.oas.models.responses.ApiResponses",
        "io.swagger.v3.oas.models.security.OAuthFlow","io.swagger.v3.oas.models.security.OAuthFlows","io.swagger.v3.oas.models.security.Scopes",
        "io.swagger.v3.oas.models.security.SecurityRequirement","io.swagger.v3.oas.models.security.SecurityScheme",
        "io.swagger.v3.oas.models.servers.Server","io.swagger.v3.oas.models.servers.ServerVariable","io.swagger.v3.oas.models.servers.ServerVariables",
        "io.swagger.v3.oas.models.tags.Tag"
    };

    static Object unwrap(Object ser) throws Exception {
        for (Class<?> c = ser.getClass(); c != null; c = c.getSuperclass()) {
            for (Field f : c.getDeclaredFields()) {
                if (f.getName().equals("defaultSerializer")) { f.setAccessible(true); return f.get(ser); }
            }
        }
        return ser;
    }

    public static void main(String[] args) throws Exception {
        Map<String, ObjectMapper> mappers = new LinkedHashMap<>();
        mappers.put("json", Json.mapper());
        mappers.put("json31", Json31.mapper());
        mappers.put("converter", Json31.converterMapper());
        mappers.put("yaml", Yaml.mapper());
        for (Map.Entry<String, ObjectMapper> m : mappers.entrySet()) {
            for (String cn : CLASSES) {
                Class<?> cls = Class.forName(cn);
                JsonSerializer<Object> ser = m.getValue().getSerializerProviderInstance().findValueSerializer(cls);
                Object inner = unwrap(ser);
                StringBuilder sb = new StringBuilder();
                sb.append(m.getKey()).append(' ').append(cls.getSimpleName()).append(' ').append(ser.getClass().getSimpleName());
                if (inner instanceof BeanSerializerBase) {
                    BeanSerializerBase b = (BeanSerializerBase) inner;
                    Iterator<PropertyWriter> it = b.properties();
                    sb.append(" [");
                    while (it.hasNext()) { PropertyWriter w = it.next(); sb.append(w.getName()); if (!w.getClass().equals(BeanPropertyWriter.class)) sb.append("#").append(w.getClass().getSimpleName());
                        if (w instanceof BeanPropertyWriter) {
                            BeanPropertyWriter bw = (BeanPropertyWriter) w;
                            Field sf = BeanPropertyWriter.class.getDeclaredField("_serializer"); sf.setAccessible(true);
                            Object s = sf.get(bw);
                            if (s != null) sb.append("{").append(s.getClass().getSimpleName()).append("}");
                            Field inc = BeanPropertyWriter.class.getDeclaredField("_suppressNulls"); inc.setAccessible(true);
                            Field sv = BeanPropertyWriter.class.getDeclaredField("_suppressableValue"); sv.setAccessible(true);
                            sb.append("<").append(inc.get(bw)).append(",").append(sv.get(bw)).append(">");
                        }
                        if (it.hasNext()) sb.append(", "); }
                    sb.append("]");
                } else sb.append(" inner=").append(inner.getClass().getName());
                System.out.println(sb);
            }
        }
    }
}
