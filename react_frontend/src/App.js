// 前端已集成在 rust_web_app/assets（由 Rust 服务静态托管）。
// 若需独立 React 构建，可将 build 产物复制到 rust_web_app/assets。
const App = () => (
  <div>
    请运行 <code>cargo run</code>（在 rust_web_app 目录）并访问 http://localhost:8080
  </div>
);

export default App;
