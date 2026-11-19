import com.fasterxml.jackson.databind.*;
import com.fasterxml.jackson.datatype.jsr310.JavaTimeModule;
import java.util.*;
public class KeyProbe {
  public static void main(String[] a) throws Exception {
    ObjectMapper m = new ObjectMapper();
    m.registerModule(new JavaTimeModule());
    m.configure(SerializationFeature.FAIL_ON_EMPTY_BEANS, false);
    m.configure(SerializationFeature.WRITE_DATES_AS_TIMESTAMPS, false);
    m.configure(SerializationFeature.WRITE_ENUMS_USING_TO_STRING, true);
    m.configure(DeserializationFeature.FAIL_ON_UNKNOWN_PROPERTIES, false);
    m.configure(SerializationFeature.WRITE_BIGDECIMAL_AS_PLAIN, true);
    Object[] keys = { null, new Date(1000), Arrays.asList("a","b"), new byte[]{1}, 1.0, true, 5L, new java.math.BigInteger("123456789012345678901234567890"), Collections.singletonMap("x", 1) };
    for (Object k : keys) {
      Map<Object,Object> mp = new LinkedHashMap<>(); mp.put(k, "v"); mp.put("n", null); mp.put("d", new Date(5)); mp.put("b", new byte[]{1,2}); mp.put("nan", Double.NaN); mp.put("s", new LinkedHashSet<>(Arrays.asList(1,2)));
      try { System.out.println(m.writeValueAsString(mp)); } catch (Exception e) { System.out.println(e.getClass().getName() + ": " + e.getMessage()); }
      try { System.out.println(io.swagger.v3.core.util.Json.mapper().convertValue(mp, JsonNode.class)); } catch (Exception e) { System.out.println(e.getClass().getName() + ": " + e.getMessage()); }
    }
  }
}
