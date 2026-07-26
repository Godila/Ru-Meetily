'use client';

import React, { createContext, useContext, useState, useEffect, useRef, useCallback } from 'react';
import { usePathname, useRouter } from 'next/navigation';
import Analytics from '@/lib/analytics';
import { invoke } from '@tauri-apps/api/core';
import { useRecordingState } from '@/contexts/RecordingStateContext';

/**
 * Summary generation status, shared between SidebarProvider (global) and
 * useSummaryGeneration (page-local). Lifted here so the Sidebar can show a
 * spinner per meeting that survives navigation.
 */
export type SummaryStatus =
  | 'idle'
  | 'processing'
  | 'summarizing'
  | 'regenerating'
  | 'completed'
  | 'error';

/** Backend statuses that map to "generation in progress" for the sidebar spinner. */
const ACTIVE_SUMMARY_STATUSES: ReadonlySet<SummaryStatus> = new Set([
  'processing',
  'summarizing',
  'regenerating',
]);

/** Map backend api_get_summary status string -> our SummaryStatus. */
function backendStatusToSummaryStatus(backendStatus: string): SummaryStatus {
  const s = backendStatus.toLowerCase();
  if (s === 'completed') return 'completed';
  if (s === 'error' || s === 'failed') return 'error';
  // pending / processing both show as "processing" spinner in the sidebar
  if (s === 'pending' || s === 'processing') return 'processing';
  return 'idle';
}


interface SidebarItem {
  id: string;
  title: string;
  type: 'folder' | 'file';
  children?: SidebarItem[];
}

export interface CurrentMeeting {
  id: string;
  title: string;
}

// Search result type for transcript search
interface TranscriptSearchResult {
  id: string;
  title: string;
  matchContext: string;
  timestamp: string;
};

interface SidebarContextType {
  currentMeeting: CurrentMeeting | null;
  setCurrentMeeting: (meeting: CurrentMeeting | null) => void;
  sidebarItems: SidebarItem[];
  isCollapsed: boolean;
  toggleCollapse: () => void;
  meetings: CurrentMeeting[];
  setMeetings: (meetings: CurrentMeeting[]) => void;
  isMeetingActive: boolean;
  setIsMeetingActive: (active: boolean) => void;
  handleRecordingToggle: () => void;
  searchTranscripts: (query: string) => Promise<void>;
  searchResults: TranscriptSearchResult[];
  isSearching: boolean;
  setServerAddress: (address: string) => void;
  serverAddress: string;
  transcriptServerAddress: string;
  setTranscriptServerAddress: (address: string) => void;
  // Summary polling management. The polling map is intentionally NOT exposed:
  // it is an internal ref only. Consumers use start/stop and summaryStatuses.
  startSummaryPolling: (meetingId: string, processId: string, onUpdate: (result: any) => void) => void;
  stopSummaryPolling: (meetingId: string) => void;
  // Per-meeting summary status — survives navigation, drives the sidebar spinner.
  summaryStatuses: Record<string, SummaryStatus>;
  setSummaryStatus: (meetingId: string, status: SummaryStatus) => void;
  // Refetch meetings from backend
  refetchMeetings: () => Promise<void>;

}

const SidebarContext = createContext<SidebarContextType | null>(null);

export const useSidebar = () => {
  const context = useContext(SidebarContext);
  if (!context) {
    throw new Error('useSidebar must be used within a SidebarProvider');
  }
  return context;
};

export function SidebarProvider({ children }: { children: React.ReactNode }) {
  const [currentMeeting, setCurrentMeeting] = useState<CurrentMeeting | null>({ id: 'intro-call', title: '+ Новая встреча' });
  const [isCollapsed, setIsCollapsed] = useState(true);
  const [meetings, setMeetings] = useState<CurrentMeeting[]>([]);
  const [sidebarItems, setSidebarItems] = useState<SidebarItem[]>([]);
  const [isMeetingActive, setIsMeetingActive] = useState(false);
  const [searchResults, setSearchResults] = useState<any[]>([]);
  const [isSearching, setIsSearching] = useState(false);
  const [serverAddress, setServerAddress] = useState('');
  const [transcriptServerAddress, setTranscriptServerAddress] = useState('');
  // Polling interval handles live in a ref (mutated synchronously inside the
  // stable polling callbacks). This avoids a state+effect mirror that could
  // briefly desync on rapid restarts and leak intervals. Nothing renders off
  // this map, so it does not need to be reactive state.
  const activeSummaryPollsRef = useRef<Map<string, ReturnType<typeof setInterval>>>(new Map());
  // Per-meeting summary status, lifted into global state so the Sidebar can show
  // a spinner that survives navigation away from the meeting-details page.
  const [summaryStatuses, setSummaryStatuses] = useState<Record<string, SummaryStatus>>({});

  // Use recording state from RecordingStateContext (single source of truth)
  const { isRecording } = useRecordingState();

  const pathname = usePathname();
  const router = useRouter();

  // Extract fetchMeetings as a reusable function
  const fetchMeetings = React.useCallback(async () => {
    if (serverAddress) {
      try {
        const meetings = await invoke('api_get_meetings') as Array<{ id: string, title: string }>;
        const transformedMeetings = meetings.map((meeting: any) => ({
          id: meeting.id,
          title: meeting.title
        }));
        setMeetings(transformedMeetings);
        Analytics.trackBackendConnection(true);
      } catch (error) {
        console.error('Error fetching meetings:', error);
        setMeetings([]);
        Analytics.trackBackendConnection(false, error instanceof Error ? error.message : 'Unknown error');
      }
    }
  }, [serverAddress]);

  useEffect(() => {
    fetchMeetings();
  }, [serverAddress, fetchMeetings]);

  useEffect(() => {
    const fetchSettings = async () => {
      setServerAddress('http://localhost:5167');
      setTranscriptServerAddress('http://127.0.0.1:8178/stream');
    };
    fetchSettings();
  }, []);

  const baseItems: SidebarItem[] = [
    {
      id: 'meetings',
      title: 'Заметки встреч',
      type: 'folder' as const,
      children: [
        ...meetings.map(meeting => ({ id: meeting.id, title: meeting.title, type: 'file' as const }))
      ]
    },
  ];


  const toggleCollapse = () => {
    setIsCollapsed(!isCollapsed);
  };

  // Update current meeting when on home page
  useEffect(() => {
    if (pathname === '/') {
      setCurrentMeeting({ id: 'intro-call', title: '+ Новая встреча' });
    }
    setSidebarItems(baseItems);
  }, [pathname]);

  // Update sidebar items when meetings change
  useEffect(() => {
    setSidebarItems(baseItems);
  }, [meetings]);

  // Function to handle recording toggle from sidebar
  const handleRecordingToggle = () => {
    if (!isRecording) {
      // Check if already on home page
      if (pathname === '/') {
        // Already on home - trigger recording directly via custom event
        console.log('Triggering recording from sidebar (already on home page)');
        window.dispatchEvent(new CustomEvent('start-recording-from-sidebar'));
      } else {
        // Not on home - navigate and use auto-start mechanism
        console.log('Navigating to home page with auto-start flag');
        sessionStorage.setItem('autoStartRecording', 'true');
        router.push('/');
      }

      // Track recording initiation from sidebar
      Analytics.trackButtonClick('start_recording', 'sidebar');
    }
    // The actual recording start/stop is handled in the Home component
  };

  // Function to search through meeting transcripts
  const searchTranscripts = async (query: string) => {
    if (!query.trim()) {
      setSearchResults([]);
      return;
    }

    try {
      setIsSearching(true);


      const results = await invoke('api_search_transcripts', { query }) as TranscriptSearchResult[];
      setSearchResults(results);
    } catch (error) {
      console.error('Error searching transcripts:', error);
      setSearchResults([]);
    } finally {
      setIsSearching(false);
    }
  };

  // Summary polling management.
  //
  // IMPORTANT: these callbacks mutate activeSummaryPollsRef synchronously (no
  // state, no effect mirror). This keeps them stable (empty dep array) AND
  // eliminates the desync window that previously could leak intervals on rapid
  // restarts of the same meeting's poll. The map is internal and never rendered.
  const stopPollInternal = useCallback((meetingId: string) => {
    const interval = activeSummaryPollsRef.current.get(meetingId);
    if (interval) {
      clearInterval(interval);
    }
    activeSummaryPollsRef.current.delete(meetingId);
  }, []);

  const startSummaryPolling = useCallback((
    meetingId: string,
    processId: string,
    onUpdate: (result: any) => void
  ) => {
    // Stop existing poll for this meeting if any (idempotent resume)
    stopPollInternal(meetingId);

    console.log(`📊 Starting polling for meeting ${meetingId}, process ${processId}`);

    // Mark as in-progress immediately so the sidebar spinner shows before the
    // first 5s poll tick returns.
    setSummaryStatuses(prev => ({ ...prev, [meetingId]: 'processing' }));

    let pollCount = 0;
    const MAX_POLLS = 200; // ~16.5 minutes at 5-second intervals (slightly longer than backend's 15-min timeout to avoid race conditions)

    const pollInterval = setInterval(async () => {
      pollCount++;

      // Timeout safety: Stop after ~16.5 minutes
      if (pollCount >= MAX_POLLS) {
        console.warn(`⏱️ Polling timeout for ${meetingId} after ${MAX_POLLS} iterations`);
        clearInterval(pollInterval);
        stopPollInternal(meetingId);
        setSummaryStatuses(prev => {
          const next = { ...prev };
          delete next[meetingId];
          return next;
        });
        onUpdate({
          status: 'error',
          error: 'Summary generation timed out after 15 minutes. Please try again or check your model configuration.'
        });
        return;
      }
      try {
        const result = await invoke('api_get_summary', {
          meetingId: meetingId,
        }) as any;

        console.log(`📊 Polling update for ${meetingId}:`, result.status);

        // Call the update callback with result
        onUpdate(result);

        // Derive our status and keep the global sidebar map in sync.
        const derived = backendStatusToSummaryStatus(result.status);
        if (ACTIVE_SUMMARY_STATUSES.has(derived)) {
          setSummaryStatuses(prev => ({ ...prev, [meetingId]: derived }));
        }

        // Stop polling if completed, error, failed, cancelled, or idle (after initial processing)
        if (result.status === 'completed' || result.status === 'error' || result.status === 'failed' || result.status === 'cancelled') {
          console.log(`Polling completed for ${meetingId}, status: ${result.status}`);
          clearInterval(pollInterval);
          stopPollInternal(meetingId);
          setSummaryStatuses(prev => {
            const next = { ...prev };
            delete next[meetingId];
            return next;
          });
        } else if (result.status === 'idle' && pollCount > 1) {
          // If we get 'idle' after polling started, process completed/disappeared
          console.log(`Process completed or not found for ${meetingId}, stopping poll`);
          clearInterval(pollInterval);
          stopPollInternal(meetingId);
          setSummaryStatuses(prev => {
            const next = { ...prev };
            delete next[meetingId];
            return next;
          });
        }
      } catch (error) {
        console.error(`Polling error for ${meetingId}:`, error);
        // Report error to callback
        onUpdate({
          status: 'error',
          error: error instanceof Error ? error.message : 'Unknown error'
        });
        clearInterval(pollInterval);
        stopPollInternal(meetingId);
        setSummaryStatuses(prev => {
          const next = { ...prev };
          delete next[meetingId];
          return next;
        });
      }
    }, 5000); // Poll every 5 seconds

    // Register synchronously so a rapid second startSummaryPolling for the same
    // meeting sees and clears this interval via stopPollInternal above.
    activeSummaryPollsRef.current.set(meetingId, pollInterval);
  }, [stopPollInternal]);

  const stopSummaryPolling = useCallback((meetingId: string) => {
    console.log(`⏹️ Stopping polling for meeting ${meetingId}`);
    stopPollInternal(meetingId);
    setSummaryStatuses(prev => {
      if (!(meetingId in prev)) return prev;
      const next = { ...prev };
      delete next[meetingId];
      return next;
    });
  }, [stopPollInternal]);

  /**
   * Manually set / clear a meeting's summary status. Used by useSummaryGeneration
   * to reflect local transitions (e.g. 'regenerating') immediately, and by the
   * meeting-details mount effect to seed the status from api_get_summary before
   * polling resumes.
   */
  const setSummaryStatus = useCallback((meetingId: string, status: SummaryStatus) => {
    setSummaryStatuses(prev => {
      if (ACTIVE_SUMMARY_STATUSES.has(status)) {
        return { ...prev, [meetingId]: status };
      }
      // Non-active status -> clear the entry
      if (!(meetingId in prev)) return prev;
      const next = { ...prev };
      delete next[meetingId];
      return next;
    });
  }, []);

  return (
    <SidebarContext.Provider value={{
      currentMeeting,
      setCurrentMeeting,
      sidebarItems,
      isCollapsed,
      toggleCollapse,
      meetings,
      setMeetings,
      isMeetingActive,
      setIsMeetingActive,
      handleRecordingToggle,
      searchTranscripts,
      searchResults,
      isSearching,
      setServerAddress,
      serverAddress,
      transcriptServerAddress,
      setTranscriptServerAddress,
      startSummaryPolling,
      stopSummaryPolling,
      summaryStatuses,
      setSummaryStatus,
      refetchMeetings: fetchMeetings,

    }}>
      {children}
    </SidebarContext.Provider>
  );
}
