import java.net.*;

class PluginLoader {
    ClassLoader load(String url) throws Exception {
        return new URLClassLoader(new URL[] {new URL(url)});
    }
}
