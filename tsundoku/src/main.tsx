import { StrictMode } from "react"
import { createRoot } from "react-dom/client"
import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { App } from "./App"
import { getAppInfo, queryKeys } from "./api"
import "./index.css"

const queryClient = new QueryClient()

// Read before the first render, since the page's background depends on whether the window has Mica.
const appInfo = await getAppInfo()
queryClient.setQueryData(queryKeys.appInfo, appInfo)
document.documentElement.dataset.backdrop = appInfo.hasMicaBackdrop ? "mica" : "solid"

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <App />
    </QueryClientProvider>
  </StrictMode>,
)
