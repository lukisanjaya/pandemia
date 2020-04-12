import 'package:bloc/bloc.dart';
import 'package:pandemia_mobile/api/pandemia_api.dart';
import 'package:pandemia_mobile/blocs/sub_report/sub_report_event.dart';
import 'package:pandemia_mobile/blocs/sub_report/sub_report_state.dart';
import 'package:pandemia_mobile/core/smart_repo.dart';
import 'package:pandemia_mobile/models/sub_report.dart';

class SubReportBloc extends Bloc<SubReportEvent, SubReportState> {
  PersistentSmartRepo repo;

  SubReportBloc() {
    repo = PersistentSmartRepo("bloc_sub_report");
  }

  @override
  SubReportState get initialState => SubReportLoading();

  @override
  Stream<SubReportState> mapEventToState(SubReportEvent event) async* {
    if (event is LoadSubReport) {
      yield* _mapLoadSubReportToState(event);
    } else if (event is CreateSubReport) {
      yield* _mapCreateSubReportToState(event);
    } else if (event is DeleteSubReport) {
      yield* _mapDeleteToState(event);
    }
  }

  Stream<SubReportState> _mapLoadSubReportToState(LoadSubReport event) async* {
    if (event.withLoading) {
      yield SubReportListLoading();
    }

    yield* repo
        .fetchGradually(
            "entries_status-${event.status}",
            () => PublicApi.get(
                "/pandemia/v1/sub_report/search?status=${event.status}&offset=0&limit=10"),
            force: event.force)
        .asyncExpand((d) async* {
      if (d != null) {
        final entries = (d.data["entries"] as List<dynamic>)
            .map((a) => SubReport.fromMap(a))
            .toList();

        if (d.isLocal) {
          yield SubReportListLoaded(entries);
        } else {
          yield SubReportListUpdated(entries);
        }
      } else {
        yield SubReportFailure(error: "Cannot get data from server");
      }
    });
  }

  Stream<SubReportState> _mapCreateSubReportToState(
      CreateSubReport event) async* {
    yield SubReportLoading();
    final payload = {
      "full_name": event.fullName,
      "age": event.age,
      "residence_address": event.residenceAddress,
      "gender": event.gender,
      "arrival_address": event.arrivalAddress,
      "arrival_date": event.arrivalDate,
      "desc": event.desc,
      "status": event.status,
      "complaint": event.complaint,
    };
    yield* PublicApi.post("/pandemia/v1/sub_report/add", payload).then((data) {
      if (data != null) {
        repo.updateEntriesItem(
            "entries_status-${event.status}", data["result"]);
        return SubReportCreated();
      } else {
        return SubReportFailure(error: "Tidak dapat menambahkan data");
      }
    }).catchError((error) {
      return SubReportFailure(error: error.toString());
    }).asStream();
    dispatch(LoadSubReport(status: event.status > 1 ? 1 : 0));
  }

  Stream<SubReportState> _mapDeleteToState(DeleteSubReport event) async* {
    yield SubReportLoading();

    final data = await PublicApi.post(
        "/sub_report/v1/delete", {"id": event.subReport.id});

    if (data != null) {
      await repo.deleteEntriesItem(
          "entries_status-${event.subReport.status}", event.subReport.toMap());

      yield SubReportDeleted(event.subReport);
      dispatch(LoadSubReport(status: event.subReport.status, force: false));
    } else {
      yield SubReportFailure(error: "Cannot delete SubReport");
    }
  }
}
