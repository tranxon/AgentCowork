import { useState } from "react";
import { AppLayout } from "./components/layout/AppLayout";
import { OnboardingFlow } from "./components/onboarding/OnboardingFlow";
import { ToastProvider } from "./components/common/ToastProvider";

function App() {
  const [onboardingDone, setOnboardingDone] = useState(() => {
    return localStorage.getItem("rollball_onboarding") === "completed";
  });

  return (
    <ToastProvider>
      {!onboardingDone ? (
        <OnboardingFlow onComplete={() => setOnboardingDone(true)} />
      ) : (
        <AppLayout />
      )}
    </ToastProvider>
  );
}

export default App;
