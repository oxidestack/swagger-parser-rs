import com.fasterxml.jackson.databind.*;
import java.nio.file.*;
import java.security.*;
// For each file argument: sha256 of Json.pretty(readTree) (or the error message) — compared by crates/jackson/examples/json_files.rs
public class JsonFiles {
  public static void main(String[] a) throws Exception {
    ObjectMapper strict = io.swagger.v3.parser.ObjectMapperFactory.createJson();
    for (String f : Files.readAllLines(Paths.get(a[0]))) {
      String s = new String(Files.readAllBytes(Paths.get(f)), "UTF-8");
      String r;
      try { r = io.swagger.v3.core.util.Json.pretty(strict.readTree(s)) + "\n" + strict.readTree(s).toString(); } catch (Exception e) { r = "ERR " + e.getMessage(); }
      byte[] d = MessageDigest.getInstance("SHA-256").digest(r.getBytes("UTF-8"));
      StringBuilder sb = new StringBuilder(); for (byte b : d) sb.append(String.format("%02x", b));
      System.out.println(sb + " " + f);
    }
  }
}
