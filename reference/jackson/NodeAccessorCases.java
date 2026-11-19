import com.fasterxml.jackson.databind.*;
import java.nio.file.*;
// Reads cases (one JSON-escaped JSON document per line) and prints the root node's asText("dflt"), asInt(-7),
// asBoolean(true), asBoolean(false), asInt(), asBoolean(): the crate's `as_*_or` accessors.
public class NodeAccessorCases {
  public static void main(String[] a) throws Exception {
    ObjectMapper plain = new ObjectMapper();
    for (String line : Files.readAllLines(Paths.get(a[0]))) {
      JsonNode n = plain.readTree(plain.readValue(line, String.class));
      if (n == null) n = com.fasterxml.jackson.databind.node.MissingNode.getInstance();
      String r = n.asText("dflt") + "|" + n.asInt(-7) + "|" + n.asBoolean(true) + "|" + n.asBoolean(false) + "|" + n.asInt() + "|" + n.asBoolean();
      System.out.println(plain.writeValueAsString(r));
    }
  }
}
