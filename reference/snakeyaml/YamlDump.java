import org.yaml.snakeyaml.*;
import org.yaml.snakeyaml.constructor.*;
import org.yaml.snakeyaml.representer.*;
import java.nio.file.*;
import java.util.*;
import java.security.*;
// Canonical typed dump of SafeConstructor output (default resolver and swagger's CustomResolver) per file,
// hashed; compared with crates/snakeyaml/examples/yaml_dump.rs. Mode "text" prints the dump itself.
public class YamlDump {
  static void dump(Object o, StringBuilder sb, IdentityHashMap<Object,Integer> seen) {
    if (o == null) { sb.append("N"); return; }
    if (o instanceof Map || o instanceof List || o instanceof Set) {
      if (seen.containsKey(o)) { sb.append("@" + seen.get(o)); return; }
      seen.put(o, seen.size());
    }
    if (o instanceof Map) { sb.append("{"); for (Object e : ((Map)o).entrySet()) { Map.Entry en=(Map.Entry)e; dump(en.getKey(), sb, seen); sb.append("="); dump(en.getValue(), sb, seen); sb.append(","); } sb.append("}"); }
    else if (o instanceof Set) { sb.append("S["); for (Object e : (Set)o) { dump(e, sb, seen); sb.append(","); } sb.append("]"); }
    else if (o instanceof List) { sb.append("["); for (Object e : (List)o) { if (e instanceof Object[]) { Object[] p=(Object[])e; sb.append("P("); dump(p[0],sb,seen); sb.append(":"); dump(p[1],sb,seen); sb.append(")"); } else dump(e, sb, seen); sb.append(","); } sb.append("]"); }
    else if (o instanceof String) sb.append("s:").append(((String)o).replace("\\","\\\\").replace("\n","\\n")).append(";");
    else if (o instanceof Integer) sb.append("i:").append(o);
    else if (o instanceof Long) sb.append("l:").append(o);
    else if (o instanceof java.math.BigInteger) sb.append("I:").append(o);
    else if (o instanceof Double) sb.append("d:").append(o);
    else if (o instanceof Boolean) sb.append("b:").append(o);
    else if (o instanceof byte[]) sb.append("B:").append(Base64.getEncoder().encodeToString((byte[])o));
    else if (o instanceof Date) sb.append("D:").append(((Date)o).getTime());
    else sb.append("?").append(o.getClass().getName());
  }
  static String run(String s, boolean custom) {
    try {
      LoaderOptions lo = io.swagger.v3.parser.util.DeserializationUtils.buildLoaderOptions();
      Yaml yaml = custom ? new Yaml(new SafeConstructor(lo), new Representer(new DumperOptions()), new DumperOptions(), lo, new io.swagger.v3.parser.util.DeserializationUtils.CustomResolver()) : new Yaml(new SafeConstructor(lo));
      StringBuilder sb = new StringBuilder(); dump(yaml.load(s), sb, new IdentityHashMap<>()); return sb.toString();
    } catch (Throwable e) { return "ERR " + e.getClass().getName() + ": " + e.getMessage(); }
  }
  public static void main(String[] a) throws Exception {
    boolean text = a.length > 1 && a[1].equals("text");
    for (String f : Files.readAllLines(Paths.get(a[0]))) {
      String s = new String(Files.readAllBytes(Paths.get(f)), "UTF-8");
      String r = run(s, true) + "\n" + run(s, false);
      if (text) { System.out.println(f + "\n" + r); continue; }
      byte[] d = MessageDigest.getInstance("SHA-256").digest(r.getBytes("UTF-8"));
      StringBuilder sb = new StringBuilder(); for (byte b : d) sb.append(String.format("%02x", b));
      System.out.println(sb + " " + f);
    }
  }
}
