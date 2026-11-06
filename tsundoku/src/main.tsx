import { StrictMode } from "react"
import { createRoot } from "react-dom/client"
import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { App } from "./App"
import {
  getAppInfo,
  getFirstRunOptions,
  getToolsNeedSetup,
  onAddingOptionsChanged,
  onToolsChanged,
  queryKeys,
} from "./api"
import { trackQueue } from "./queueEvents"
import { trackToolProgress } from "./toolProgress"
import "./index.css"

const queryClient = new QueryClient()

// Read before the first render: the page's background depends on whether the window has Mica, and the first screen on
// whether the tools still need setting up.
const [appInfo, needsSetup] = await Promise.all([getAppInfo(), getToolsNeedSetup()])
queryClient.setQueryData(queryKeys.appInfo, appInfo)
queryClient.setQueryData(queryKeys.toolsNeedSetup, needsSetup)
// The setup opens on its choices when they're still to be made.
if (needsSetup) queryClient.setQueryData(queryKeys.firstRunOptions, await getFirstRunOptions())
document.documentElement.dataset.backdrop = appInfo.hasMicaBackdrop ? "mica" : "solid"

trackToolProgress()
trackQueue(queryClient)
void onToolsChanged(() => void queryClient.invalidateQueries({ queryKey: queryKeys.toolsOverview }))
// The tray's Watch Clipboard changes the options outside the UI.
void onAddingOptionsChanged((options) => queryClient.setQueryData(queryKeys.addingOptions, options))

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <App />
    </QueryClientProvider>
  </StrictMode>,
)
