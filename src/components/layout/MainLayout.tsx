import { PanelGroup, Panel, PanelResizeHandle } from 'react-resizable-panels';
import Sidebar from '../sidebar/Sidebar';
import TabsArea from '../editor/TabsArea';
import TitleBar from './TitleBar';

export default function MainLayout() {
  return (
    <div className="flex flex-col h-screen bg-[var(--bg-primary)] overflow-hidden">
      <TitleBar />
      <div className="flex-1 overflow-hidden">
        <PanelGroup direction="horizontal">
          <Panel defaultSize={22} minSize={15} maxSize={35}>
            <Sidebar />
          </Panel>
          <PanelResizeHandle className="w-px bg-[var(--border)] hover:bg-brand-500 transition-colors cursor-col-resize" />
          <Panel defaultSize={78} minSize={50}>
            <TabsArea />
          </Panel>
        </PanelGroup>
      </div>
    </div>
  );
}
