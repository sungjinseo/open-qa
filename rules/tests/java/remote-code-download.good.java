class PluginLoader {
    Class<?> load(String name) throws Exception {
        return Class.forName(name);
    }
}
