import javax.script.*;

class Calc {
    Object run(String expr) throws ScriptException {
        ScriptEngine engine = new ScriptEngineManager().getEngineByName("js");
        return engine.eval(expr);
    }
}
