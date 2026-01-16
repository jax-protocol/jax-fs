import { Route } from "@solidjs/router";
import { Component, ParentComponent } from "solid-js";
import Sidebar from "./components/Sidebar";
import BucketsList from "./pages/BucketsList";
import BucketExplorer from "./pages/BucketExplorer";
import FileViewer from "./pages/FileViewer";

// Layout wrapper for routes
const Layout: ParentComponent = (props) => {
  return (
    <div class="flex h-screen bg-gray-100 dark:bg-gray-900">
      <Sidebar />
      <main class="flex-1 overflow-auto">{props.children}</main>
    </div>
  );
};

const App: Component = () => {
  return (
    <Route path="/" component={Layout}>
      <Route path="/" component={BucketsList} />
      <Route path="/buckets/:id" component={BucketExplorer} />
      <Route path="/buckets/:id/view/*path" component={FileViewer} />
    </Route>
  );
};

export default App;
