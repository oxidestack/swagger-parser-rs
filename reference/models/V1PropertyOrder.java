import com.fasterxml.jackson.databind.*;
import com.fasterxml.jackson.databind.ser.*;
import com.fasterxml.jackson.databind.ser.std.*;
import io.swagger.v3.core.util.*;
import java.lang.reflect.*;
import java.util.*;

public class V1PropertyOrder {
    static String[] CLASSES = {"io.swagger.models.AbstractModel","io.swagger.models.ArrayModel","io.swagger.models.BooleanValueModel","io.swagger.models.ComposedModel","io.swagger.models.Contact","io.swagger.models.ExternalDocs","io.swagger.models.HttpMethod","io.swagger.models.Info","io.swagger.models.License","io.swagger.models.Model","io.swagger.models.ModelImpl","io.swagger.models.Operation","io.swagger.models.Path","io.swagger.models.RefModel","io.swagger.models.RefPath","io.swagger.models.RefResponse","io.swagger.models.Response","io.swagger.models.Responses","io.swagger.models.Scheme","io.swagger.models.SecurityRequirement","io.swagger.models.SecurityScope","io.swagger.models.Swagger","io.swagger.models.Tag","io.swagger.models.Xml","io.swagger.models.auth.AbstractSecuritySchemeDefinition","io.swagger.models.auth.ApiKeyAuthDefinition","io.swagger.models.auth.AuthorizationValue","io.swagger.models.auth.BasicAuthDefinition","io.swagger.models.auth.In","io.swagger.models.auth.OAuth2Definition","io.swagger.models.auth.SecuritySchemeDefinition","io.swagger.models.auth.UrlMatcher","io.swagger.models.parameters.AbstractParameter","io.swagger.models.parameters.AbstractSerializableParameter","io.swagger.models.parameters.BodyParameter","io.swagger.models.parameters.CookieParameter","io.swagger.models.parameters.FormParameter","io.swagger.models.parameters.HeaderParameter","io.swagger.models.parameters.Parameter","io.swagger.models.parameters.PathParameter","io.swagger.models.parameters.QueryParameter","io.swagger.models.parameters.RefParameter","io.swagger.models.parameters.SerializableParameter","io.swagger.models.properties.AbstractNumericProperty","io.swagger.models.properties.AbstractProperty","io.swagger.models.properties.ArrayProperty","io.swagger.models.properties.BaseIntegerProperty","io.swagger.models.properties.BinaryProperty","io.swagger.models.properties.BooleanProperty","io.swagger.models.properties.BooleanValueProperty","io.swagger.models.properties.ByteArrayProperty","io.swagger.models.properties.ComposedProperty","io.swagger.models.properties.DateProperty","io.swagger.models.properties.DateTimeProperty","io.swagger.models.properties.DecimalProperty","io.swagger.models.properties.DoubleProperty","io.swagger.models.properties.EmailProperty","io.swagger.models.properties.FileProperty","io.swagger.models.properties.FloatProperty","io.swagger.models.properties.IntegerProperty","io.swagger.models.properties.LongProperty","io.swagger.models.properties.MapProperty","io.swagger.models.properties.ObjectProperty","io.swagger.models.properties.PasswordProperty","io.swagger.models.properties.Property","io.swagger.models.properties.PropertyBuilder","io.swagger.models.properties.RefProperty","io.swagger.models.properties.StringProperty","io.swagger.models.properties.UUIDProperty","io.swagger.models.properties.UntypedProperty"};

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
        mappers.put("v1json", io.swagger.util.Json.mapper());
        for (Map.Entry<String, ObjectMapper> m : mappers.entrySet()) {
            for (String cn : CLASSES) {
                Class<?> cls = Class.forName(cn); if (cls.isInterface() || java.lang.reflect.Modifier.isAbstract(cls.getModifiers()) || cls.isEnum()) continue;
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
