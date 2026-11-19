import com.fasterxml.jackson.databind.*;
import java.nio.file.*;
import java.security.*;
// sha256 of Yaml.mapper().readTree(file) toString (or error) per file; see crates/jackson/examples/yaml_files.rs
public class YamlFiles {
  public static void main(String[] a) throws Exception {
    ObjectMapper y = io.swagger.v3.core.util.Yaml.mapper();
    for (String f : Files.readAllLines(Paths.get(a[0]))) {
      String s = new String(Files.readAllBytes(Paths.get(f)), "UTF-8");
      String r;
      try { r = y.readTree(s).toString(); } catch (Throwable e) { r = "ERR " + e.getMessage(); }
      if (a.length > 1) { System.out.println(f + "\n" + r); continue; }
      byte[] d = MessageDigest.getInstance("SHA-256").digest(r.getBytes("UTF-8"));
      StringBuilder sb = new StringBuilder(); for (byte b : d) sb.append(String.format("%02x", b));
      System.out.println(sb + " " + f);
    }
  }
}
