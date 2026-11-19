import com.fasterxml.jackson.databind.*;
import com.fasterxml.jackson.databind.node.*;
import java.math.*;
import java.util.*;
public class NodeText {
  public static void main(String[] a) throws Exception {
    JsonNodeFactory f = JsonNodeFactory.instance;
    JsonNode[] ns = { f.numberNode(1.0), f.numberNode(1e20), f.numberNode(Double.NaN), f.numberNode(1.5f), f.numberNode(new BigDecimal("1E+3")), f.numberNode(new BigDecimal("0.00001")), f.binaryNode(new byte[]{1,2,3}), f.textNode("a\"b\u0001\u007f/é"), f.nullNode(), MissingNode.getInstance(), f.booleanNode(true), f.numberNode((short)3), f.numberNode(new BigInteger("123456789012345678901234567890")), f.pojoNode(new Date(0)), f.pojoNode(UUID.fromString("123e4567-e89b-12d3-a456-426614174000")) };
    for (JsonNode n : ns) System.out.println(n.getClass().getSimpleName() + " toString=" + n + " asText=" + n.asText() + " json=" + io.swagger.v3.core.util.Json.mapper().writeValueAsString(n)+ " obj=" + io.swagger.v3.core.util.Json.mapper().convertValue(n, Object.class) + " pretty=" + io.swagger.v3.core.util.Json.pretty(n));
    ObjectNode o = f.objectNode(); o.put("a", 1); o.putArray("b").add(1).add("x").addObject(); o.putObject("c"); o.putArray("d");
    System.out.println(io.swagger.v3.core.util.Json.pretty(o));
    System.out.println(new ObjectMapper().writerWithDefaultPrettyPrinter().writeValueAsString(o));
    Map<String,Object> m = new LinkedHashMap<>(); m.put("a", null); m.put("b", 1); m.put("d", new Date(1000)); m.put("o", java.time.OffsetDateTime.parse("2020-01-02T03:04:05.500+02:00")); m.put("bd", new BigDecimal("1E+3"));
    System.out.println(io.swagger.v3.core.util.Json.mapper().writeValueAsString(m));
    System.out.println(new ObjectMapper().writeValueAsString(Collections.singletonMap("a", null)));
    System.out.println(io.swagger.v3.core.util.Json.mapper().valueToTree(m).toString());
    System.out.println(m.toString().replace(new Date(1000).toString(), "DATE"));
    System.out.println(io.swagger.v3.core.util.Json.mapper().convertValue(io.swagger.v3.core.util.Json.mapper().readTree("{\"a\":1.5,\"b\":[1,2147483648]}"), Object.class));
    System.out.println(new Date(0) + " TZ=" + TimeZone.getDefault().getID());
  }
}
